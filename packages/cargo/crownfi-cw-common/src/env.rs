use std::{rc::Rc, cell::RefCell};

use cosmwasm_std::{Env, Deps, DepsMut, Storage, Api, CustomQuery, Empty, QuerierWrapper};

// common immutable-storage trait?
#[derive(Clone)]
pub struct ClonableEnvInfo<'exec, Q: CustomQuery = Empty> {
	pub storage: Rc<&'exec dyn Storage>,
	pub api: Rc<&'exec dyn Api>,
	pub querier: Rc<QuerierWrapper<'exec, Q>>,
	pub env: Rc<Env>
}
impl<'exec, Q: CustomQuery> ClonableEnvInfo<'exec, Q> {
	pub fn new(deps: Deps<'exec, Q>, env: Env) -> Self {
		ClonableEnvInfo {
			storage: Rc::new(deps.storage),
			api: Rc::new(deps.api),
			querier: Rc::new(deps.querier),
			env: Rc::new(env)
		}
	}
}

/// Wraps Env and Deps
#[derive(Clone)]
pub struct ClonableEnvInfoMut<'exec, Q: CustomQuery = Empty> {
	pub storage: Rc<RefCell<&'exec mut dyn Storage>>,
	pub api: Rc<&'exec dyn Api>,
	pub querier: Rc<QuerierWrapper<'exec, Q>>,
	pub env: Rc<Env>
}

impl<'exec, Q: CustomQuery> ClonableEnvInfoMut<'exec, Q> {
	pub fn new(deps: DepsMut<'exec, Q>, env: Env) -> Self {
		ClonableEnvInfoMut {
			storage: Rc::new(RefCell::new(deps.storage)),
			api: Rc::new(deps.api),
			querier: Rc::new(deps.querier),
			env: Rc::new(env)
		}
	}
}
