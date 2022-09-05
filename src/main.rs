use crate::lib::Rpc;
use mullvad_management_interface::types::daemon_event::Event;
use mullvad_management_interface::types::{DaemonEvent, TunnelState};
use mullvad_management_interface::ManagementServiceClient;
use std::time::Duration;
use tokio::select;
use tokio::time::sleep;
use tokio_stream::StreamExt;

mod lib;

#[tokio::main]
async fn main() {
    let mut mullvad: ManagementServiceClient = mullvad_management_interface::new_rpc_client()
        .await
        .expect("Could not create mullvad rpc client. Is the mullvad daemon running?");
    let mut event_stream = mullvad
        .events_listen(())
        .await
        .expect("Could not listen to mullvad events!")
        .into_inner();
    let mut rpc = Rpc::new().await;
    let mut poll_interval = Duration::from_secs(0);
    loop {
        let tunnel_state_maybe =
            poll_tunnel_update(&mut mullvad, &mut event_stream, poll_interval).await;
        if let Some(tunnel_state) = tunnel_state_maybe {
            println!("Updating discord status...");
            if let Err(error) = rpc.update_tunnel_state(&tunnel_state).await {
                println!("Could not update rpc: {:?}", error);
            }
        }
        poll_interval = Duration::from_secs(10);
    }
}

async fn poll_tunnel_update(
    mullvad: &mut ManagementServiceClient,
    event_stream: &mut tonic::codec::Streaming<DaemonEvent>,
    poll_interval: Duration,
) -> Option<TunnelState> {
    select! {
        resp = event_stream.next() => {
            match resp {
                Some(Ok(DaemonEvent {
                    event: Some(Event::TunnelState(tunnel_state)),
                })) => Some(tunnel_state),
                Some(Err(status)) => {
                    eprintln!("Could not get tunnel state: {}", status);
                    None
                }
                _ => None,
            }
        },
        () = sleep(poll_interval) => {
            match mullvad.get_tunnel_state(()).await {
                Ok(tunnel_resp) => Some(tunnel_resp.into_inner()),
                Err(status) => {
                    eprintln!("Could not get tunnel state: {}", status);
                    None
                }
            }
        },
    }
}
