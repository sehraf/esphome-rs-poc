#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum MessageTypes {
    HelloResponse = 2,
    ConnectResponse = 4,
    DisconnectResponse = 5,
    PingResponse = 8,
    DeviceInfoResponse = 10,
    ListEntitiesLightResponse = 15,
    ListEntitiesSensorResponse = 16,
    ListEntitiesDoneResponse = 19,
    LightStateResponse = 24,
    SensorStateResponse = 25,
    SubscribeLogsResponse = 29,
}
