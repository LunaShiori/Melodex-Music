//! 极简 MQTT v5 客户端。
//!
//! QQ 音乐的扫码登录不提供「查询扫码状态」的 HTTP 接口（`GetQRCodeStatus` 实测恒返回
//! `status: "scaned"`，`scanUser.userID` 永远为 0，是占位值而非真实状态）。扫码事件
//! 只能从 `wss://mu.y.qq.com/ws/handshake` 上以 MQTT 消息推送过来，因此这里实现
//! MQTT 5 中登录流程真正用到的那几个报文：
//!
//! - **CONNECT** —— 带 `tmeAppID` / `business` / `hashTag` / `clientTag` / `userID`
//!   五个 User Property，服务端据此把连接绑定到某个二维码会话。
//! - **SUBSCRIBE** —— 订阅 `management.qrcode_login/{qrcodeID}`，带
//!   `authorization=tmelogin` / `pubsub=unicast` 两个属性。
//! - **PUBLISH** —— 服务端下发的事件。`type` 属性区分 `scanned` / `cookies` /
//!   `timeout` / `canceled` / `loginFailed`，`cookies` 事件的载荷才是登录令牌。
//! - **CONNACK** —— 可能返回 `UseAnotherServer` / `ServerMoved`，需要读
//!   `server_reference` 属性换节点重连。
//!
//! 刻意不引第三方 MQTT 库：只需要编解码这四种报文，自带的实现体量更小，也能对每个
//! 字节做单元测试（协议层一旦编错，表现是「连上了但永远收不到事件」，极难排查）。

/// 报文字节上限。扫码事件的载荷很小，超过这个长度说明协议解析错位了。
const MAX_FRAME: usize = 256 * 1024;

/// MQTT 5 的属性 ID（只列本模块会读到的）。
const PROP_SERVER_REFERENCE: u8 = 0x1c;
const PROP_USER_PROPERTY: u8 = 0x26;
const PROP_AUTHENTICATION_METHOD: u8 = 0x15;

/// `Use another server` / `Server moved` 两个原因码，都要求按 `server_reference` 重连。
pub const REASON_USE_ANOTHER_SERVER: u8 = 0x9c;
pub const REASON_SERVER_MOVED: u8 = 0x9d;

/// MQTT 的剩余长度使用 7 位一组的变长整数，最多 4 字节。
pub fn encode_varint(mut value: usize, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value % 128) as u8;
        value /= 128;
        if value > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

pub fn decode_varint(bytes: &[u8], pos: &mut usize) -> Option<usize> {
    let mut multiplier = 1usize;
    let mut value = 0usize;
    for _ in 0..4 {
        let byte = *bytes.get(*pos)?;
        *pos += 1;
        value += usize::from(byte & 0x7f) * multiplier;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        multiplier *= 128;
    }
    None
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u16).to_be_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn read_str(bytes: &[u8], pos: &mut usize) -> Option<String> {
    let len = u16::from_be_bytes([*bytes.get(*pos)?, *bytes.get(*pos + 1)?]) as usize;
    *pos += 2;
    let slice = bytes.get(*pos..*pos + len)?;
    *pos += len;
    String::from_utf8(slice.to_vec()).ok()
}

fn read_bytes(bytes: &[u8], pos: &mut usize) -> Option<Vec<u8>> {
    let len = u16::from_be_bytes([*bytes.get(*pos)?, *bytes.get(*pos + 1)?]) as usize;
    *pos += 2;
    let slice = bytes.get(*pos..*pos + len)?;
    *pos += len;
    Some(slice.to_vec())
}

/// 一个 User Property 编码为 `0x26 + 键 + 值`。
fn push_user_properties(out: &mut Vec<u8>, props: &[(&str, &str)]) {
    for (key, value) in props {
        out.push(PROP_USER_PROPERTY);
        push_str(out, key);
        push_str(out, value);
    }
}

/// 把固定头 + 变长长度 + 报文体的三段拼起来。
fn frame(header: u8, body: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 5);
    out.push(header);
    encode_varint(body.len(), &mut out);
    out.extend(body);
    out
}

/// CONNECT 报文。`clean_start=true`，这样每次登录都是全新会话。
pub fn build_connect(client_id: &str, keep_alive: u16, user_props: &[(&str, &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(b"\x00\x04MQTT");
    body.push(5); // 协议版本
    body.push(0x02); // clean start
    body.extend_from_slice(&keep_alive.to_be_bytes());

    let mut props = Vec::new();
    // QQ 侧要求带认证方式，服务端不校验内容，但不能缺
    props.push(PROP_AUTHENTICATION_METHOD);
    push_str(&mut props, "pass");
    push_user_properties(&mut props, user_props);
    encode_varint(props.len(), &mut body);
    body.extend(props);

    push_str(&mut body, client_id);
    frame(0x10, body)
}

/// SUBSCRIBE 报文，QoS 0（登录事件丢了就等重连，不需要重传队列）。
pub fn build_subscribe(pkid: u16, topic: &str, user_props: &[(&str, &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&pkid.to_be_bytes());

    let mut props = Vec::new();
    push_user_properties(&mut props, user_props);
    encode_varint(props.len(), &mut body);
    body.extend(props);

    push_str(&mut body, topic);
    body.push(0x00); // QoS 0
    frame(0x82, body)
}

/// 解析出的属性：`(server_reference, user_properties)`。
type ParsedProperties = (Option<String>, Vec<(String, String)>);

/// 解析属性段，返回 [`ParsedProperties`]。
///
/// 未识别的属性必须按类型**正确跳过**，否则后续字节会整体错位，
/// 表现为「能连上但解析出来的都是乱码」。
fn parse_properties(bytes: &[u8], pos: &mut usize) -> Option<ParsedProperties> {
    let len = decode_varint(bytes, pos)?;
    let end = pos.checked_add(len)?;
    if end > bytes.len() {
        return None;
    }
    let mut server_reference = None;
    let mut user_props = Vec::new();
    while *pos < end {
        let id = *bytes.get(*pos)?;
        *pos += 1;
        match id {
            PROP_USER_PROPERTY => {
                let key = read_str(bytes, pos)?;
                let value = read_str(bytes, pos)?;
                user_props.push((key, value));
            }
            PROP_SERVER_REFERENCE => {
                server_reference = read_str(bytes, pos);
            }
            PROP_AUTHENTICATION_METHOD => {
                let _ = read_str(bytes, pos)?;
            }
            // 单字节
            0x01 | 0x17 | 0x19 | 0x24 | 0x25 | 0x28 | 0x29 | 0x2a => {
                *pos += 1;
            }
            // 两字节
            0x13 | 0x21 | 0x22 | 0x23 => {
                *pos += 2;
            }
            // 四字节
            0x02 | 0x11 | 0x18 | 0x27 => {
                *pos += 4;
            }
            // 变长整数
            0x0b => {
                decode_varint(bytes, pos)?;
            }
            // UTF-8 字符串
            0x03 | 0x08 | 0x12 | 0x1a | 0x1f => {
                let _ = read_str(bytes, pos)?;
            }
            // 二进制
            0x09 | 0x16 => {
                let _ = read_bytes(bytes, pos)?;
            }
            // 未知属性无法确定长度，只能放弃这个包
            _ => return None,
        }
    }
    *pos = end;
    Some((server_reference, user_props))
}

/// 服务端下发的一条 PUBLISH。
#[derive(Debug, Clone)]
pub struct Publish {
    pub topic: String,
    pub payload: Vec<u8>,
    pub user_props: Vec<(String, String)>,
}

impl Publish {
    /// 取 User Property，例如 `type` = `cookies`。
    pub fn prop(&self, key: &str) -> Option<&str> {
        self.user_props
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
}

/// 解析后的一帧。
#[derive(Debug)]
pub enum Frame {
    ConnAck {
        code: u8,
        server_reference: Option<String>,
    },
    SubAck {
        pkid: u16,
    },
    Publish(Publish),
    /// PINGRESP、DISCONNECT 等本模块不关心的报文
    Other(u8),
}

/// 从缓冲区头部解析一帧，返回 `(帧, 这一帧占用的字节数)`。
///
/// 返回 `None` 表示数据不完整（需要继续读）或协议错位。
pub fn decode_frame(bytes: &[u8]) -> Option<(Frame, usize)> {
    let kind = *bytes.first()?;
    let mut pos = 1;
    let len = decode_varint(bytes, &mut pos)?;
    if len > MAX_FRAME {
        return None;
    }
    let start = pos;
    let end = start.checked_add(len)?;
    let body = bytes.get(start..end)?;

    let frame = match kind & 0xf0 {
        0x20 => {
            // CONNACK: 连接确认标志 + 原因码 + 属性
            let code = *body.get(1)?;
            let mut cursor = 2;
            let (server_reference, _) =
                parse_properties(body, &mut cursor).unwrap_or((None, Vec::new()));
            Frame::ConnAck {
                code,
                server_reference,
            }
        }
        0x90 => {
            // SUBACK: 包标识 + 属性 + 原因码列表
            let pkid = u16::from_be_bytes([*body.first()?, *body.get(1)?]);
            Frame::SubAck { pkid }
        }
        0x30 => {
            let mut cursor = 0;
            let topic = read_str(body, &mut cursor)?;
            // QoS > 0 时才有包标识，登录事件是 QoS 0，这里仍然兼容处理
            if (kind >> 1) & 0x03 > 0 {
                cursor += 2;
            }
            let (_, user_props) = parse_properties(body, &mut cursor)?;
            Frame::Publish(Publish {
                topic,
                payload: body.get(cursor..).unwrap_or_default().to_vec(),
                user_props,
            })
        }
        other => Frame::Other(other),
    };
    Some((frame, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trips() {
        for value in [0usize, 1, 127, 128, 16_383, 16_384, 2_097_151, 268_435_455] {
            let mut buf = Vec::new();
            encode_varint(value, &mut buf);
            let mut pos = 0;
            assert_eq!(decode_varint(&buf, &mut pos), Some(value));
            assert_eq!(pos, buf.len());
        }
    }

    #[test]
    fn varint_rejects_truncated() {
        // 0x80 表示「还有后续字节」，只有一个字节时应判定为不完整
        assert_eq!(decode_varint(&[0x80], &mut 0), None);
    }

    #[test]
    fn connect_carries_required_user_properties() {
        let packet = build_connect(
            "1234567",
            45,
            &[
                ("tmeAppID", "qqmusic"),
                ("business", "management"),
                ("hashTag", "QID"),
            ],
        );
        assert_eq!(packet[0], 0x10);

        let text = packet.windows(4).any(|w| w == b"MQTT");
        assert!(text);
        // 协议版本必须是 5
        let idx = packet.windows(4).position(|w| w == b"MQTT").unwrap();
        assert_eq!(packet[idx + 4], 5);
        // 三个属性键都要能原样找到
        for key in ["tmeAppID", "business", "hashTag", "qqmusic", "management"] {
            assert!(
                packet.windows(key.len()).any(|w| w == key.as_bytes()),
                "缺少 {key}"
            );
        }
    }

    #[test]
    fn subscribe_encodes_topic_and_qos() {
        let packet = build_subscribe(
            1,
            "management.qrcode_login/ABC",
            &[("authorization", "tmelogin"), ("pubsub", "unicast")],
        );
        assert_eq!(packet[0], 0x82);
        let topic = b"management.qrcode_login/ABC";
        let at = packet
            .windows(topic.len())
            .position(|w| w == topic)
            .unwrap();
        // 主题后面紧跟一字节订阅选项，QoS 0
        assert_eq!(packet[at + topic.len()], 0x00);
    }

    /// 手工拼一个 `type=cookies` 的 PUBLISH，验证属性与载荷都能取出来。
    #[test]
    fn decodes_publish_with_user_property() {
        let payload = br#"{"cookies":{"qqmusic_uin":"123","qqmusic_key":"KEY"}}"#;
        let mut body = Vec::new();
        push_str(&mut body, "management.qrcode_login/QID");
        let mut props = Vec::new();
        props.push(PROP_USER_PROPERTY);
        push_str(&mut props, "type");
        push_str(&mut props, "cookies");
        encode_varint(props.len(), &mut body);
        body.extend(props);
        body.extend_from_slice(payload);

        let mut packet = vec![0x30];
        encode_varint(body.len(), &mut packet);
        packet.extend(body);

        let (frame, used) = decode_frame(&packet).expect("应能解析");
        assert_eq!(used, packet.len());
        match frame {
            Frame::Publish(publish) => {
                assert_eq!(publish.prop("type"), Some("cookies"));
                assert_eq!(publish.topic, "management.qrcode_login/QID");
                assert_eq!(publish.payload, payload);
            }
            other => panic!("应为 PUBLISH，实际 {other:?}"),
        }
    }

    #[test]
    fn decodes_connack_success() {
        // CONNACK: 标志 0 + 原因码 0 + 空属性
        let packet = vec![0x20, 0x03, 0x00, 0x00, 0x00];
        match decode_frame(&packet) {
            Some((Frame::ConnAck { code, .. }, used)) => {
                assert_eq!(code, 0);
                assert_eq!(used, packet.len());
            }
            other => panic!("应为 CONNACK，实际 {other:?}"),
        }
    }

    /// ServerMoved 时服务端会带 server_reference，必须解析出来才能重连到正确节点。
    #[test]
    fn decodes_connack_server_moved() {
        let mut body = vec![0x00, REASON_SERVER_MOVED];
        let mut props = Vec::new();
        props.push(PROP_SERVER_REFERENCE);
        push_str(&mut props, "mu2.y.qq.com");
        encode_varint(props.len(), &mut body);
        body.extend(props);

        let mut packet = vec![0x20];
        encode_varint(body.len(), &mut packet);
        packet.extend(body);

        match decode_frame(&packet) {
            Some((
                Frame::ConnAck {
                    code,
                    server_reference,
                },
                _,
            )) => {
                assert_eq!(code, REASON_SERVER_MOVED);
                assert_eq!(server_reference.as_deref(), Some("mu2.y.qq.com"));
            }
            other => panic!("应为 CONNACK，实际 {other:?}"),
        }
    }

    /// 未识别的属性必须被正确跳过，后续的 User Property 不能错位。
    #[test]
    fn unknown_properties_are_skipped_by_type() {
        let mut body = vec![0x00, 0x00];
        let mut props = Vec::new();
        // 0x11 = Session Expiry Interval，四字节
        props.push(0x11);
        props.extend_from_slice(&3600u32.to_be_bytes());
        // 0x13 = Server Keep Alive，两字节
        props.push(0x13);
        props.extend_from_slice(&60u16.to_be_bytes());
        // 0x1f = Reason String，字符串
        props.push(0x1f);
        push_str(&mut props, "ok");
        // 最后才是 User Property
        props.push(PROP_USER_PROPERTY);
        push_str(&mut props, "type");
        push_str(&mut props, "scanned");
        encode_varint(props.len(), &mut body);
        body.extend(props);

        let mut packet = vec![0x20];
        encode_varint(body.len(), &mut packet);
        packet.extend(body);

        match decode_frame(&packet) {
            Some((Frame::ConnAck { .. }, _)) => {}
            other => panic!("应为 CONNACK，实际 {other:?}"),
        }
    }

    /// 数据不完整时不能 panic，必须返回 None 让调用方继续读。
    #[test]
    fn incomplete_frame_returns_none() {
        let packet = vec![0x30, 0x10, 0x00, 0x01];
        assert!(decode_frame(&packet).is_none());
        assert!(decode_frame(&[]).is_none());
    }
}
