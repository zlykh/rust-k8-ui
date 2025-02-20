use std::net::SocketAddr;
use std::thread;
use crossbeam::channel::{Receiver, Sender};
use eframe::egui::TextBuffer;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client};
use tokio::net::TcpListener;
use tokio::runtime::{Runtime};
use crate::k8ui::appstate::ShortKContainer;
use crate::k8ui::k8api;
use crate::k8ui::k8api::{KubeApis, refresh_apis, refresh_client, refresh_pod_list};

#[derive(Debug)]
pub enum ThreadMessage {
    Api(ApiCommand),
    Data(UIData),
}

#[derive(Debug)]
pub enum UIData {
    Pods(Vec<ShortKContainer>),
    Logs(Vec<String>),
}

#[derive(Debug)]
pub enum ApiCommand {
    ReloadClientWithConfig(String),
    ReloadApisWithNameSpace(String),
    PullPodsWithPrefix(String),
    PullLogsForPodName(String),
    PortForwardForPodNamePort(String, u16),
}

pub struct ApiThread {
    pub thread: thread::JoinHandle<()>,
}

impl ApiThread {
    pub fn new(thread_receiver: Receiver<ThreadMessage>, ui_sender: Sender<ThreadMessage>) -> Self {
        let runtime = Runtime::new().unwrap();
        let thread = thread::spawn(move || runtime.block_on(async {
            let mut i = 0;
            println!("Thread start");
            let mut client: Option<Client> = None;
            let mut apis: Option<KubeApis> = None;


            loop {

                match thread_receiver.recv() {
                    Ok(cmd) => {
                        match cmd {
                            ThreadMessage::Api(cmd) => {
                                println!("recv! {:?}", cmd);
                                api_command_matcher(cmd, &mut client, &mut apis, &ui_sender).await;
                            }
                            _ => println!("This shouldn't come to the thread"),
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }

                i += 1;
            }

            println!("Thread exit");
        }));

        Self { thread }
    }
}

async fn api_command_matcher(cmd: ApiCommand, client: &mut Option<Client>, apis: &mut Option<KubeApis>, ui_sender: &Sender<ThreadMessage>) {
    match cmd {
        ApiCommand::ReloadClientWithConfig(path) => {
            println!("refreshing client {}", path);
            *client = Some(refresh_client(path.as_str()).await);
        }

        ApiCommand::ReloadApisWithNameSpace(namespace) => {
            println!("refreshing apis {}", namespace);
            *apis = Some(refresh_apis(&client.as_ref().unwrap(), namespace.as_str()).await);
        }

        ApiCommand::PullPodsWithPrefix(prefix) => {
            println!("pulling pods {}", prefix);
            match refresh_pod_list(prefix.as_str(), &apis.as_ref().unwrap().api_pods, &apis.as_ref().unwrap().api_cfm, &apis.as_ref().unwrap().api_secrets).await {
                Ok(ui_pods) => ui_sender.try_send(ThreadMessage::Data(UIData::Pods(ui_pods))).unwrap(),
                Err(e) => println!("Error generating pod ui data {}", e),
            }
        }

        ApiCommand::PullLogsForPodName(pod_name) => {
            println!("pulling logs {}", pod_name);
            match k8api::logs(pod_name, &apis.as_ref().unwrap().api_pods).await {
                Ok(lines) => ui_sender.try_send(ThreadMessage::Data(UIData::Logs(lines))).unwrap(),
                Err(e) => println!("Error pulling logs {}", e),
            }
        }

        ApiCommand::PortForwardForPodNamePort(pod_name, port) => {
            println!("forwarding {}:{}", pod_name, port);
            port_forward(pod_name.as_str(), port, &apis.as_ref().unwrap().api_pods).await;//todo blocks!!!!!
        }
    }
}

async fn port_forward(pod_name: &str, pod_port: u16, api_pods: &Api<Pod>) {
    let addr = SocketAddr::from(([127, 0, 0, 1], pod_port + 1));
    let g: &'static str = pod_name.to_string().clone().leak();
    let bind = TcpListener::bind(addr).await.unwrap();
    let server = tokio_stream::wrappers::TcpListenerStream::new(bind)
        .take_until(tokio::signal::ctrl_c())
        .try_for_each(|client_conn| async {
            if let Ok(peer_addr) = client_conn.peer_addr() {
                println!("new conn {}", peer_addr);
            }
            let api_clone = api_pods.clone();
            tokio::spawn(async move {
                if let Err(e) = k8api::forward_connection(api_clone, g, pod_port, client_conn).await {
                    println!("failed to forward connection {}", error = e.as_ref() as &dyn std::error::Error);
                }
            });
            // keep the server running
            Ok(())
        });

    if let Err(e) = server.await {
        println!("server error {}", error = &e as &dyn std::error::Error);
    }
}