use flint_core::test_spec::ActionType;
use flint_core::timeline::TimelineAggregate;

trait TestServer {
    fn flint_do_tick(&self);
    fn flint_place_block(&self);
    fn flint_update_block(&self);
    fn flint_break_block(&self);
}

struct Simulator<'a> {
    server: Box<dyn TestServer>,
    timeline: TimelineAggregate<'a>,
}

impl Simulator<'_> {
    pub fn new(server: impl TestServer + 'static, timeline: TimelineAggregate) -> Simulator {
        Simulator {
            server: Box::new(server),
            timeline,
        }
    }

    pub fn run(&self) {
        let mut next_do_tick;
        for tick in 0..self.timeline.max_tick {
            next_do_tick = self.timeline.next_action_tick(tick);
            if next_do_tick.is_some() && next_do_tick == Some(tick) {
                // TODO Place
                if let Some(vec) = self.timeline.timeline.get(&tick) {
                    for (test_idx, timeline_entry, value_idx) in vec {
                        match &timeline_entry.action_type
                        {
                            ActionType::Place { pos, block } => {}
                            ActionType::PlaceEach { blocks } => {}
                            ActionType::Fill { region, with } => {}
                            ActionType::Remove { pos } => {}
                            ActionType::Assert { checks } => {}
                            ActionType::AssertState { pos, state, values } => {}
                        }
                    }
                }
            }
            self.server.flint_do_tick();
        }
    }
}
