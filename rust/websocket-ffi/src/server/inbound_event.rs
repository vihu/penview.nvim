use nvim_oxi::{
    Object,
    conversion::ToObject,
    mlua::{IntoLua, Lua, Result as LuaResult, Value as LuaValue},
};
use uuid::Uuid;

#[derive(Clone)]
pub enum WebsocketServerError {
    ClientTermination(Uuid, String),
    ServerTermination(String),
    ReceiveMessage(Uuid, String),
    SendMessage(Uuid, String),
    BroadcastMessage(String),
}

#[derive(Clone)]
pub enum WebsocketServerInboundEvent {
    ClientConnected(Uuid),
    ClientDisconnected(Uuid),
    NewMessage(Uuid, String),
    Error(WebsocketServerError),
}

// Not necessary (for now)
impl ToObject for WebsocketServerError {
    fn to_object(self) -> Result<Object, nvim_oxi::conversion::Error> {
        match self {
            WebsocketServerError::ClientTermination(_client_id, message) => {
                Ok(Object::from(message))
            }
            WebsocketServerError::ReceiveMessage(_client_id, message) => Ok(Object::from(message)),
            WebsocketServerError::SendMessage(_client_id, message) => Ok(Object::from(message)),
            WebsocketServerError::BroadcastMessage(message) => Ok(Object::from(message)),
            WebsocketServerError::ServerTermination(message) => Ok(Object::from(message)),
        }
    }
}

impl IntoLua for WebsocketServerError {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let vec = match self {
            WebsocketServerError::ClientTermination(client_id, message) => {
                vec![
                    ("type", "client_termination_error"),
                    ("client_id", client_id.to_string().leak()),
                    ("message", message.leak()),
                ]
            }
            WebsocketServerError::ReceiveMessage(client_id, message) => {
                vec![
                    ("type", "receive_message_error"),
                    ("client_id", client_id.to_string().leak()),
                    ("message", message.leak()),
                ]
            }
            WebsocketServerError::SendMessage(client_id, message) => {
                vec![
                    ("type", "send_message_error"),
                    ("client_id", client_id.to_string().leak()),
                    ("message", message.leak()),
                ]
            }
            WebsocketServerError::BroadcastMessage(message) => {
                vec![
                    ("type", "broadcast_message_error"),
                    ("message", message.leak()),
                ]
            }
            WebsocketServerError::ServerTermination(message) => {
                vec![
                    ("type", "server_termination_error"),
                    ("message", message.leak()),
                ]
            }
        };
        Ok(LuaValue::Table(lua.create_table_from(vec)?))
    }
}
