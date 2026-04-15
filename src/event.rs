use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

#[derive(Debug, Clone)]
pub enum Event {
    Key(KeyEvent),
    Tick,
}

pub struct EventStream {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventStream {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let tick_tx = tx.clone();

        tokio::spawn(async move {
            let mut tick_interval = interval(Duration::from_millis(250));
            loop {
                tick_interval.tick().await;
                if tick_tx.send(Event::Tick).is_err() {
                    break;
                }
            }
        });

        let input_tx = tx;
        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    if let Ok(CrosstermEvent::Key(key)) = event::read() {
                        if input_tx.send(Event::Key(key)).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

impl Default for EventStream {
    fn default() -> Self {
        Self::new()
    }
}
