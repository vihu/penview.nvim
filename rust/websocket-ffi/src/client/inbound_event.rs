use nvim_oxi::{
    mlua::{IntoLua, Lua, Result as LuaResult, Value as LuaValue},
    {Object, conversion::ToObject},
};

#[derive(Clone, Debug)]
pub enum WebsocketClientError {
    Connection(String),
    ReceiveMessage(String),
    SendMessage(String),
}

#[derive(Clone, Debug)]
pub enum WebsocketClientInboundEvent {
    Connected,
    Disconnected,
    NewMessage(String),
    Error(WebsocketClientError),
}

impl ToObject for WebsocketClientError {
    fn to_object(self) -> Result<Object, nvim_oxi::conversion::Error> {
        match self {
            WebsocketClientError::Connection(message) => Ok(Object::from(message)),
            WebsocketClientError::ReceiveMessage(message) => Ok(Object::from(message)),
            WebsocketClientError::SendMessage(message) => Ok(Object::from(message)),
        }
    }
}

impl IntoLua for WebsocketClientError {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let vec = match self {
            WebsocketClientError::Connection(message) => {
                vec![("type", "connection_error"), ("message", message.leak())]
            }
            WebsocketClientError::ReceiveMessage(message) => {
                vec![
                    ("type", "receive_message_error"),
                    ("message", message.leak()),
                ]
            }
            WebsocketClientError::SendMessage(message) => {
                vec![("type", "send_message_error"), ("message", message.leak())]
            }
        };
        Ok(LuaValue::Table(lua.create_table_from(vec)?))
    }
}
