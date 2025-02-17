use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use arci::{
    gamepad::{Button, Gamepad, GamepadEvent},
    Speaker,
};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, warn};

use super::control_node::ControlNode;

pub struct ControlNodeSwitcher<S>
where
    S: Speaker,
{
    current_index: Arc<Mutex<usize>>,
    control_nodes: Arc<TokioMutex<Vec<Arc<dyn ControlNode>>>>,
    speaker: S,
    is_running: Arc<AtomicBool>,
}

impl<S> ControlNodeSwitcher<S>
where
    S: Speaker,
{
    #[track_caller]
    pub fn new(
        control_nodes: Vec<Arc<dyn ControlNode>>,
        speaker: S,
        initial_node_index: usize,
    ) -> Self {
        assert!(!control_nodes.is_empty());
        Self {
            current_index: Arc::new(Mutex::new(initial_node_index)),
            control_nodes: Arc::new(TokioMutex::new(control_nodes)),
            speaker,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn increment_mode(&self) -> Result<(), arci::Error> {
        let len = self.control_nodes.lock().await.len();
        {
            let mut index = self.current_index.lock().unwrap();
            *index = (*index + 1) % len;
        }
        self.speak_current_mode().await
    }

    pub async fn speak_current_mode(&self) -> Result<(), arci::Error> {
        let nodes = self.control_nodes.lock().await;
        let i = self.current_index();
        let mode = nodes[i].mode();
        let submode = nodes[i].submode();
        self.speaker.speak(&format!("{}{}", mode, submode))?.await
    }

    fn current_index(&self) -> usize {
        *self.current_index.lock().unwrap()
    }

    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
    }

    pub async fn main<G>(&self, gamepad: G)
    where
        G: 'static + Gamepad,
    {
        let nodes = self.control_nodes.clone();
        let index = self.current_index.clone();
        let is_running = self.is_running.clone();
        self.is_running.store(true, Ordering::Relaxed);
        self.speak_current_mode().await.unwrap();
        let gamepad = Arc::new(gamepad);
        let gamepad_cloned = gamepad.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            while is_running.load(Ordering::Relaxed) {
                debug!("tick");
                let node = { nodes.lock().await[*index.lock().unwrap()].clone() };
                node.proc().await;
                interval.tick().await;
            }
            gamepad_cloned.stop();
        });
        while self.is_running() {
            let ev = gamepad.next_event().await;
            debug!("event: {:?}", ev);
            match ev {
                GamepadEvent::ButtonPressed(Button::North) => {
                    self.increment_mode().await.unwrap();
                }
                GamepadEvent::Unknown => {
                    warn!("gamepad Unknown");
                    self.stop();
                }
                _ => {
                    let node = { self.control_nodes.lock().await[self.current_index()].clone() };
                    node.handle_event(ev);
                }
            }
        }
    }
}
