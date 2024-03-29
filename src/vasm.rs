use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::dsl::FlowDsl;
use crate::petri_net::PetriNet;

/// RoleMap is a type alias for a HashMap that maps a string to a boolean.
pub type RoleMap = HashMap<String, bool>;

/// Vector is a type alias for a vector of 32-bit integers.
/// It is used to represent the state of a state machine and the delta of each transition or inhibitor.
pub type Vector = Vec<i32>;

/// ModelType is an enum that represents the type of model.
/// It is used to determine the type of state machine to use.
/// The possible values are `PetriNet`, `Elementary`, and `Workflow`.
/// The default value is `PetriNet`.
/// The `Elementary` model is a simplified version of the `PetriNet` model.
/// The `Workflow` model is a simplified version of the `Elementary` model.
/// The `PetriNet` model is the most complex and general model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    PetriNet,
    Elementary,
    Workflow,
}

/// Guard is a struct that represents a guard in a state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guard {
    delta: Vector,
    read: bool,
}

/// GuardMap is a type alias for a HashMap that maps a string to a `Guard`.
pub type GuardMap = HashMap<String, Guard>;

/// Transition is a struct that represents a transition in a state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    label: String,
    role: String,
    delta: Vector,
    guards: GuardMap,
    allow_reentry: bool,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            label: "".to_string(),
            role: "".to_string(),
            delta: vec![],
            guards: GuardMap::new(),
            allow_reentry: false,
        }
    }
}

/// TransitionMap is a type alias for a HashMap that maps a string to a `Transition`.
pub type TransitionMap = HashMap<String, Transition>;

/// StateMachine is a struct that holds the vectorized / executable form of a Petri-net.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachine {
    pub model_type: ModelType,
    pub initial: Vector,
    pub capacity: Vector,
    pub places: Vec<String>,
    pub transitions: TransitionMap,
    pub roles: RoleMap,
}

fn model_type_from_string(model_type: &str) -> ModelType {
    match model_type {
        "elementary" => ModelType::Elementary,
        "workflow" => ModelType::Workflow,
        _ => ModelType::PetriNet,
    }
}

fn vector_add(capacity: &Vector, state: &Vector, delta: &Vector, multiple: i32) -> (Vector, bool, bool, bool) {
    let mut overflow = false;
    let mut underflow = false;
    let mut output: Vector = Vec::new();
    let mut ok = true;
    for i in 0..state.len() {
        output.push(state[i] + delta.get(i).unwrap_or(&0) * multiple);
        if output[i] < 0 {
            underflow = true;
            ok = false; // underflow: contains negative
        } else if capacity[i] > 0 && capacity[i] - output[i] < 0 {
            overflow = true;
            ok = false; // overflow: exceeds capacity
        }
    }
    (output, ok, overflow, underflow)
}

impl StateMachine {
    /// Creates a new `StateMachine` object from the given `PetriNet`.
    pub fn new(declaration: fn(&mut dyn FlowDsl)) -> Self {
        let net = &mut PetriNet::new();
        let mut sm = net.declare(declaration).as_vasm();
        sm.model_type = model_type_from_string(&net.model_type);
        return sm;
    }

    /// Creates a new `StateMachine` object from the given `PetriNet`.
    pub fn from_model(model: &mut PetriNet) -> Self {
        let model_type = model_type_from_string(&model.model_type);
        model.populate_arc_attributes();
        let mut roles = RoleMap::new();
        model.transitions.iter().for_each(|(_, v)| {
            roles.insert(v.role.clone().unwrap_or("default".to_string()), true);
        });

        let vector_size = model.places.len();

        let mut transitions: TransitionMap = model
            .transitions
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    Transition {
                        label: k.clone(),
                        role: v.role.clone().unwrap_or("default".to_string()),
                        delta: vec![0; vector_size],
                        guards: GuardMap::new(),
                        allow_reentry: false,
                    },
                )
            })
            .collect();

        model.arcs.iter().for_each(|arc| {
            let source = arc.source.clone();
            let target = arc.target.clone();
            let weight = arc.weight.unwrap_or(1);
            let consume = arc.consume.unwrap_or(false);
            let produce = arc.produce.unwrap_or(false);
            let inhibit = arc.inhibit.unwrap_or(false);
            let read = arc.read.unwrap_or(false);

            let p = if read || produce {
                model.places.get(&target)
            } else {
                model.places.get(&source)
            }.unwrap();

            let t = if read || produce {
                transitions.get_mut(&source)
            } else {
                transitions.get_mut(&target)
            }.unwrap();

            let delta = &mut vec![0; vector_size];
            delta[p.offset as usize] = 0 - weight;

            if inhibit {
                t.guards.insert(
                    target.clone(),
                    Guard { delta: delta.clone(), read },
                );
            } else {
                if consume {
                    t.delta[model.places.get(&source).unwrap().offset as usize] = 0 - weight;
                } else {
                    t.delta[model.places.get(&target).unwrap().offset as usize] = weight;
                }
            }
        });

        let mut initial = vec![0; vector_size];
        let mut capacity = vec![0; vector_size];
        let mut places = vec!["".to_string(); vector_size];

        model.places.iter().for_each(|(k, v)| {
            let i = v.initial.unwrap_or(0);
            assert!(i >= 0, "initial must be non-negative");

            initial[v.offset as usize] = match model_type {
                ModelType::PetriNet => i,
                ModelType::Elementary => match i {
                    0 => 0,
                    _ => 1,
                },
                ModelType::Workflow => match i {
                    0 => 0,
                    _ => 1,
                },
            };

            capacity[v.offset as usize] = match model_type {
                ModelType::PetriNet => v.capacity.unwrap_or(0),
                ModelType::Elementary => 1,
                ModelType::Workflow => 1,
            };
            places[v.offset as usize] = k.clone();
        });

        Self {
            model_type: model_type_from_string(&model.model_type),
            initial,
            capacity,
            places,
            transitions,
            roles,
        }
    }

    /// Checks if any guard fails for the given state and transition.
    fn guard_fails(&self, state: &Vector, transition: &Transition, multiple: i32) -> Result<bool, &'static str> {
        for (_, guard) in &transition.guards {
            let (_, threshold_met, _, _) = vector_add(&self.capacity, state, &guard.delta, multiple);
            return if guard.read {
                Ok(!threshold_met) // read arc enables after a threshold
            } else {
                Ok(threshold_met) // guard inhibits until a threshold
            };
        }
        Ok(false)
    }
    pub fn petri_net_fire(&self, state: &Vector, transition: &Transition, multiple: i32) -> Transaction {
        let role = transition.role.clone();
        let (output, ok, overflow, underflow) = vector_add(&self.capacity, state, &transition.delta, multiple);
        let inhibited = self.guard_fails(state, transition, multiple).unwrap();

        Transaction {
            output,
            ok: ok && !inhibited,
            role,
            inhibited,
            overflow,
            underflow,
        }
    }

    pub fn elementary_fire(&self, state: &Vector, transition: &Transition, multiple: i32) -> Transaction {
        let role = transition.role.clone();
        let (output, ok, overflow, underflow) = vector_add(&self.capacity, state, &transition.delta, multiple);
        let inhibited = self.guard_fails(state, transition, multiple).unwrap_or(false);
        let output_state_count = output.iter().filter(|&x| *x > 0).count();
        let elementary_ok = ok && output_state_count == 1 && !inhibited;
        Transaction {
            output,
            ok: elementary_ok,
            role,
            inhibited,
            overflow,
            underflow,
        }
    }

    pub fn workflow_fire(&self, state: &Vector, transition: &Transition, multiple: i32) -> Transaction {
        let role = transition.role.clone();
        let (output, ok, overflow, underflow) = vector_add(&self.capacity, state, &transition.delta, multiple);
        let inhibited = self.guard_fails(state, transition, multiple).unwrap();
        let workflow_output = output.iter().map(|x| {
            match x {
                -1 => 0, // allow retry / reentry
                0 => 0,
                1 => 1,
                2 => 1, // allow reentry
                _ => 1, // no other values allowed
            }
        }).collect::<Vec<i32>>();
        let output_state_count = workflow_output.iter().filter(|&x| *x > 0).count();
        if !inhibited && overflow && output_state_count == 1 && transition.allow_reentry {
            return Transaction {
                output: workflow_output,
                ok: true,
                role,
                inhibited,
                overflow: false,
                underflow,
            };
        }
        let workflow_ok = ok && output_state_count == 1 && !inhibited;

        Transaction {
            output,
            ok: workflow_ok,
            role,
            inhibited,
            overflow,
            underflow,
        }
    }
}

/// `Transaction` is a struct that represents the result of a transformation in a state machine.
/// It provides information about the success of the transformation, the resulting state, the role that performed the transformation, and any errors that occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// A boolean indicating whether the transformation was successful.
    pub ok: bool,
    /// The resulting state after the transformation.
    pub output: Vector,
    /// The role that performed the transformation.
    pub role: String,
    /// An optional boolean indicating whether the transformation was inhibited.
    pub inhibited: bool,
    /// An optional boolean indicating whether an overflow occurred during the transformation.
    pub overflow: bool,
    /// An optional boolean indicating whether an underflow occurred during the transformation.
    pub underflow: bool,
}

impl Transaction {
    /// Checks if the transaction was successful.
    ///
    /// # Returns
    ///
    /// * A boolean indicating whether the transaction was successful.
    ///
    pub fn is_ok(&self) -> bool {
        self.ok
    }

    pub fn is_err(&self) -> bool {
        !self.ok
    }
}

/// `Vasm` is a trait that represents a [vector addition state machine](https://en.wikipedia.org/wiki/Vector_addition_system).
/// It provides methods to create an empty vector, get the initial Vector, and transform the state.
pub trait Vasm {
    /// Creates an empty vector with the same length as the number of places in the state machine.
    ///
    /// # Returns
    ///
    /// * A vector of zeros with the same length as the number of places in the state machine.
    fn empty_vector(&self) -> Vector;

    /// Gets the initial vector of the state machine.
    ///
    /// # Returns
    ///
    /// * The initial vector of the state machine.
    ///
    fn initial_vector(&self) -> Vector;

    /// Transforms the state of the state machine according to the given action and multiple.
    ///
    /// # Arguments
    ///
    /// * `state` - The current state of the state machine.
    /// * `action` - The action to be performed.
    /// * `multiple` - The multiple of the action to be performed.
    ///
    /// # Returns
    ///
    /// * A `Transaction` object that represents the result of the transformation.
    ///
    fn transform(&self, state: &Vector, action: &str, multiple: i32) -> Transaction;
}

impl dyn Vasm {
    pub fn new(declaration: fn(&mut dyn FlowDsl)) -> Box<Self> {
        Box::from(PetriNet::new().declare(declaration).as_vasm())
    }
}

impl Vasm for StateMachine {
    fn empty_vector(&self) -> Vector {
        vec![0; self.places.len()]
    }

    fn initial_vector(&self) -> Vector {
        self.initial.clone()
    }

    // REVIEW: test that this works properly
    fn transform(&self, state: &Vector, action: &str, multiple: i32) -> Transaction {
        let transition = self
            .transitions
            .get(action)
            .unwrap_or_else(|| panic!("no transition for {}", action));

        match self.model_type {
            ModelType::PetriNet => self.petri_net_fire(state, transition, multiple),
            ModelType::Elementary => self.elementary_fire(state, transition, multiple),
            ModelType::Workflow => self.workflow_fire(state, transition, multiple),
        }
    }
}

#[test]
fn test_default_net() {
    let net = &mut PetriNet::new();
    let mut mm = net.declare(|m| {
        m.model_type("petriNet");
    });
    let vasm = mm.as_vasm();
    let state = vasm.initial_vector();
    assert!(state.len() == 0);
}
