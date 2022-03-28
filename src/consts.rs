use std::fmt::{Display, Formatter, Result};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageTypes {
    Unkown = 0,
    HelloRequest = 1,
    HelloResponse,
    ConnectRequest,
    ConnectResponse,
    DisconnectRequest,
    DisconnectResponse,
    PingRequest,
    PingResponse,
    DeviceInfoRequest,
    DeviceInfoResponse, // = 10
    ListEntitiesRequest,
    ListEntitiesBinarySensorResponse,
    ListEntitiesCoverResponse,
    ListEntitiesFanResponse,
    ListEntitiesLightResponse,
    ListEntitiesSensorResponse,
    ListEntitiesSwitchResponse,
    ListEntitiesTextSensorResponse,
    ListEntitiesDoneResponse,
    SubscribeStatesRequest, // = 20
    BinarySensorStateResponse,
    CoverStateResponse,
    FanStateResponse,
    LightStateResponse,
    SensorStateResponse,
    SwitchStateResponse,
    TextSensorStateResponse,
    SubscribeLogsRequest,
    SubscribeLogsResponse,
    CoverCommandRequest, // = 30
    FanCommandRequest,
    LightCommandRequest,
    SwitchCommandRequest,
    SubscribeHomeassistantServicesRequest,
    HomeassistantServiceResponse,
    GetTimeRequest,
    GetTimeResponse,
    SubscribeHomeAssistantStatesRequest,
    SubscribeHomeAssistantStateResponse,
    HomeAssistantStateResponse, // = 40
}

impl From<u32> for MessageTypes {
    fn from(ty: u32) -> Self {
        match ty {
            1 => Self::HelloRequest,
            2 => Self::HelloResponse,
            3 => Self::ConnectRequest,
            4 => Self::ConnectResponse,
            5 => Self::DisconnectRequest,
            6 => Self::DisconnectResponse,
            7 => Self::PingRequest,
            8 => Self::PingResponse,
            9 => Self::DeviceInfoRequest,
            10 => Self::DeviceInfoResponse,
            11 => Self::ListEntitiesRequest,
            12 => Self::ListEntitiesBinarySensorResponse,
            13 => Self::ListEntitiesCoverResponse,
            14 => Self::ListEntitiesFanResponse,
            15 => Self::ListEntitiesLightResponse,
            16 => Self::ListEntitiesSensorResponse,
            17 => Self::ListEntitiesSwitchResponse,
            18 => Self::ListEntitiesTextSensorResponse,
            19 => Self::ListEntitiesDoneResponse,
            20 => Self::SubscribeStatesRequest,
            21 => Self::BinarySensorStateResponse,
            22 => Self::CoverStateResponse,
            23 => Self::FanStateResponse,
            24 => Self::LightStateResponse,
            25 => Self::SensorStateResponse,
            26 => Self::SwitchStateResponse,
            27 => Self::TextSensorStateResponse,
            28 => Self::SubscribeLogsRequest,
            29 => Self::SubscribeLogsResponse,
            30 => Self::CoverCommandRequest,
            31 => Self::FanCommandRequest,
            32 => Self::LightCommandRequest,
            33 => Self::SwitchCommandRequest,
            34 => Self::SubscribeHomeassistantServicesRequest,
            35 => Self::HomeassistantServiceResponse,
            36 => Self::GetTimeRequest,
            37 => Self::GetTimeResponse,
            38 => Self::SubscribeHomeAssistantStatesRequest,
            39 => Self::SubscribeHomeAssistantStateResponse,
            40 => Self::HomeAssistantStateResponse,
            _ => Self::Unkown,
        }
    }
}

impl Display for MessageTypes {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{} [{:?}]", *self as u32, self)
    }
}
