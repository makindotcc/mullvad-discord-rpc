pub use discord_sdk as ds;
use discord_sdk::activity::{ActivityBuilder, Assets};
use discord_sdk::Subscriptions;
use mullvad_management_interface::types::{tunnel_state, TunnelState, TunnelStateRelayInfo};
use std::time::SystemTime;

pub const APP_ID: ds::AppId = 1015745829954404362;

pub async fn connect_to_discord() -> ds::Discord {
    let (wheel, handler) = ds::wheel::Wheel::new(Box::new(|err| {
        eprintln!("Encountered an error in discord wheel: {:?}", err);
    }));
    let mut user = wheel.user();
    let discord = ds::Discord::new(
        ds::DiscordApp::PlainId(APP_ID),
        Subscriptions::empty(),
        Box::new(handler),
    )
    .unwrap();

    println!("Waiting for discord handshake...");
    user.0.changed().await.unwrap();
    let user = match &*user.0.borrow() {
        ds::wheel::UserState::Connected(user) => user.clone(),
        ds::wheel::UserState::Disconnected(err) => {
            panic!("failed to connect to Discord: {:?}", err)
        }
    };
    println!("Connected to Discord, local user is {:#?}", user);

    discord
}

pub struct Rpc {
    discord: ds::Discord,
    state: RpcState,
}

impl Rpc {
    pub async fn new() -> Rpc {
        let discord_client = connect_to_discord().await;
        Rpc {
            discord: discord_client,
            state: RpcState::Inactive,
        }
    }

    pub async fn update_tunnel_state(
        &mut self,
        tunnel_state: &TunnelState,
    ) -> Result<(), ds::Error> {
        match &tunnel_state.state {
            Some(tunnel_state::State::Connected(tunnel_state::Connected {
                relay_info: Some(relay_info),
            })) => {
                let active = self.state.update_relay(relay_info.clone());
                let activity = build_activity(relay_info).start_timestamp(active.started_at);
                self.discord.update_activity(activity).await?;
                self.state = RpcState::Active(active);
                Ok(())
            }
            _ => {
                if let RpcState::Active(_) = self.state {
                    self.state = RpcState::Inactive;
                    self.discord.clear_activity().await?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
struct RpcActive {
    started_at: SystemTime,
    relay_info: TunnelStateRelayInfo,
}

impl RpcActive {
    fn start_now(relay_info: TunnelStateRelayInfo) -> RpcActive {
        RpcActive {
            started_at: SystemTime::now(),
            relay_info,
        }
    }
}

#[derive(Debug)]
enum RpcState {
    Inactive,
    Active(RpcActive),
}

impl RpcState {
    fn update_relay(&self, relay_info: TunnelStateRelayInfo) -> RpcActive {
        match self {
            RpcState::Inactive => RpcActive::start_now(relay_info),
            RpcState::Active(RpcActive {
                relay_info: prev_relay_info,
                ..
            }) if prev_relay_info != &relay_info => RpcActive::start_now(relay_info),
            RpcState::Active(active) => active.clone(),
        }
    }
}

fn build_activity(relay_info: &TunnelStateRelayInfo) -> ActivityBuilder {
    ActivityBuilder::default()
        .assets(Assets::default().large("mullvad", Some("Mullvad VPN")))
        .details("Host: 195149233153.lobez.plusnet.pl")
        .state(activity_state(&relay_info))
}

fn activity_state(relay_info: &TunnelStateRelayInfo) -> String {
    match &relay_info.location {
        Some(mullvad_management_interface::types::GeoIpLocation {
            country,
            city,
            hostname,
            ..
        }) => format!("Exit: {} ({}/{})", hostname, country, city),
        _ => String::from("Secure connection"),
    }
}
