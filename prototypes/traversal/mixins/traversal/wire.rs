// The messages between the prototype's client and server halves, as JSON on two mod channels.
use crate::game::trv_measure::ClientStats;
use crate::game::trv_plans::{Plan, Rope};
use crate::game::trv_rules::{Hit, Kind, Method, V3};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const TO_SERVER: &str = "rc_traversal:c2s";
pub const TO_CLIENT: &str = "rc_traversal:s2c";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToServer {
    /// Start an attempt; `hit` is the block aimed at, for the kinds that aim.
    Start {
        kind: Kind,
        method: Method,
        hit: Option<Hit>,
    },
    /// Method a: the client began following the plan.
    Started {
        attempt: u64,
    },
    /// Method a: the client finished following the plan.
    Done {
        attempt: u64,
        stats: ClientStats,
    },
    Stop,
    Clear,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToClient {
    /// The request reached the server; the attempt starts at its next level tick.
    Queued,
    Refused {
        reason: String,
    },
    Accepted {
        attempt: u64,
        kind: Kind,
        method: Method,
        planned: u32,
        length: f64,
    },
    /// Method a: the plan the client follows itself.
    Follow {
        attempt: u64,
        plan: Plan,
    },
    /// The attempt ended; `line` is the server's summary.
    Finished {
        attempt: u64,
        line: String,
    },
    /// A rope to draw, or None to take it away.
    Rope {
        id: u64,
        rope: Option<Rope>,
    },
    /// A harpoon line from the puller to the point, or None to take it away. `mine`: the
    /// puller is this client's player.
    Hook {
        entity: i32,
        mine: bool,
        point: Option<V3>,
    },
    /// Another player's attempt started near this one: log how the rider moves on this screen.
    Watch {
        attempt: u64,
        entity: i32,
        kind: Kind,
        method: Method,
        label: String,
    },
    Unwatch {
        attempt: u64,
    },
}

pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    // Plain data with string keys: serialising it cannot fail.
    serde_json::to_vec(message).unwrap_or_default()
}

pub fn decode<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    serde_json::from_slice(data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::trv_plans::plan_zip;

    #[test]
    fn messages_survive_the_trip() {
        let hit = Hit {
            block: [20, 72, 4],
            point: [20.0, 72.5, 4.5],
        };
        let start = ToServer::Start {
            kind: Kind::Zip,
            method: Method::B2,
            hit: Some(hit),
        };
        assert_eq!(decode::<ToServer>(&encode(&start)), Ok(start));
        let plan = plan_zip([0.3, 64.0, 0.7], &hit, true).unwrap();
        let follow = ToClient::Follow {
            attempt: 3,
            plan: plan.clone(),
        };
        let bytes = encode(&follow);
        assert!(bytes.len() < 30_000, "a mod message holds 30 000 bytes");
        // serde_json may round a float's last bit on the way back, so points are compared
        // within a hair.
        let Ok(ToClient::Follow {
            attempt: 3,
            plan: back,
        }) = decode::<ToClient>(&bytes)
        else {
            panic!("the plan did not come back");
        };
        assert_eq!(
            (back.legs.len(), back.climb, back.gate.is_some()),
            (plan.legs.len(), plan.climb, plan.gate.is_some())
        );
        let near = |a: &[V3], b: &[V3]| {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(p, q)| crate::game::trv_rules::distance(*p, *q) < 1e-9)
        };
        assert!(near(back.path.points(), plan.path.points()));
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(decode::<ToClient>(b"{\"Nope\":1}").is_err());
        assert!(decode::<ToServer>(&[0xff, 0x00]).is_err());
    }
}
