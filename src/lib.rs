//! Stream Processor — real-time A2A deliberation stream
//! Carries intent deltas, agent proposals, confidence scores, equilibrium signals.

use std::collections::{HashMap, VecDeque};

/// Events that flow through the deliberation stream
#[derive(Debug, Clone)]
pub enum StreamEvent {
    IntentDelta { goal: String, confidence: f64 },
    Proposal { agent: String, approach: String, confidence: f64 },
    Consideration { agent: String, proposal_id: u64, verdict: Verdict, reason: String },
    Forfeit { agent: String, to_agent: String, confidence_transferred: f64 },
    ConvergenceSignal { confidence: f64, threshold: f64, converged: bool },
    FeedbackSignal { source: String, adjustment: f64, constraint_violation: Option<String> },
    EquilibriumReached { final_confidence: f64, rounds: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict { Accept, Reject, Defer }

/// A stream processor — consumes and produces stream events
pub struct StreamProcessor {
    buffer: VecDeque<StreamEvent>,
    subscribers: Vec<Box<dyn Fn(&StreamEvent) -> StreamEvent>>,
    processed_count: usize,
    event_counts: HashMap<String, usize>,
}

impl StreamProcessor {
    pub fn new() -> Self {
        Self { buffer: VecDeque::new(), subscribers: vec![], processed_count: 0, event_counts: HashMap::new() }
    }

    /// Emit an event into the stream
    pub fn emit(&mut self, event: StreamEvent) {
        self.buffer.push_back(event);
    }

    /// Process all pending events through subscribers
    pub fn process(&mut self) -> Vec<StreamEvent> {
        let mut outputs = vec![];
        while let Some(event) = self.buffer.pop_front() {
            self.processed_count += 1;
            let event_type = match &event {
                StreamEvent::IntentDelta { .. } => "intent_delta",
                StreamEvent::Proposal { .. } => "proposal",
                StreamEvent::Consideration { .. } => "consideration",
                StreamEvent::Forfeit { .. } => "forfeit",
                StreamEvent::ConvergenceSignal { .. } => "convergence",
                StreamEvent::FeedbackSignal { .. } => "feedback",
                StreamEvent::EquilibriumReached { .. } => "equilibrium",
            };
            *self.event_counts.entry(event_type.to_string()).or_insert(0) += 1;

            let mut transformed = event;
            for sub in &self.subscribers {
                transformed = sub(&transformed);
            }
            outputs.push(transformed);
        }
        outputs
    }

    /// Add a subscriber that transforms events
    pub fn subscribe(&mut self, f: Box<dyn Fn(&StreamEvent) -> StreamEvent>) {
        self.subscribers.push(f);
    }

    /// Get stream statistics
    pub fn stats(&self) -> StreamStats {
        StreamStats { processed: self.processed_count, buffered: self.buffer.len(), event_counts: self.event_counts.clone() }
    }

    /// Peek at next event without consuming
    pub fn peek(&self) -> Option<&StreamEvent> { self.buffer.front() }
}

#[derive(Debug, Clone)]
pub struct StreamStats {
    pub processed: usize,
    pub buffered: usize,
    pub event_counts: HashMap<String, usize>,
}

/// Feed-in processor — user interaction feeds back into deliberation
pub struct FeedInProcessor {
    feedback_queue: VecDeque<StreamEvent>,
    adaptation_rate: f64,
}

impl FeedInProcessor {
    pub fn new() -> Self { Self { feedback_queue: VecDeque::new(), adaptation_rate: 0.1 } }

    /// User clicked/hovered — feed signal into stream
    pub fn user_feedback(&mut self, source: &str, confidence_adj: f64) {
        self.feedback_queue.push_back(StreamEvent::FeedbackSignal {
            source: source.to_string(), adjustment: confidence_adj, constraint_violation: None,
        });
    }

    /// Extract feedback events for the main stream
    pub fn drain(&mut self) -> Vec<StreamEvent> {
        self.feedback_queue.drain(..).collect()
    }

    /// Get accumulated adaptation (how much user feedback has adjusted things)
    pub fn total_adaptation(&self) -> f64 {
        self.feedback_queue.len() as f64 * self.adaptation_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_and_process() {
        let mut sp = StreamProcessor::new();
        sp.emit(StreamEvent::IntentDelta { goal: "sort".to_string(), confidence: 0.5 });
        sp.emit(StreamEvent::Proposal { agent: "arch".to_string(), approach: "sorted()".to_string(), confidence: 0.9 });
        let results = sp.process();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_subscriber() {
        let mut sp = StreamProcessor::new();
        sp.subscribe(Box::new(|e| match e {
            StreamEvent::IntentDelta { confidence, .. } => StreamEvent::IntentDelta { goal: "transformed".to_string(), confidence: *confidence * 0.9 },
            other => other.clone(),
        }));
        sp.emit(StreamEvent::IntentDelta { goal: "test".to_string(), confidence: 1.0 });
        let results = sp.process();
        match &results[0] {
            StreamEvent::IntentDelta { goal, confidence } => {
                assert_eq!(goal, "transformed");
                assert!((confidence - 0.9).abs() < 0.01);
            }
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn test_stats() {
        let mut sp = StreamProcessor::new();
        sp.emit(StreamEvent::Proposal { agent: "a".to_string(), approach: "x".to_string(), confidence: 0.5 });
        sp.emit(StreamEvent::Forfeit { agent: "a".to_string(), to_agent: "b".to_string(), confidence_transferred: 0.1 });
        sp.process();
        let stats = sp.stats();
        assert_eq!(stats.processed, 2);
    }

    #[test]
    fn test_feedback_processor() {
        let mut fp = FeedInProcessor::new();
        fp.user_feedback("click", 0.05);
        fp.user_feedback("hover", 0.02);
        let events = fp.drain();
        assert_eq!(events.len(), 2);
        assert!(fp.drain().is_empty());
    }
}
