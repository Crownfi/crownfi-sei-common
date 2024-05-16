use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{Api, CustomQuery, Deps, DepsMut, Empty, Env, QuerierWrapper, Storage};

#[derive(Clone)]
pub struct MinimalEnvInfo<'exec, Q: CustomQuery = Empty> {
	pub querier: Rc<QuerierWrapper<'exec, Q>>,
	pub env: Rc<Env>,
}
impl<'exec, Q: CustomQuery> MinimalEnvInfo<'exec, Q> {
	pub fn from_deps(deps: Deps<'exec, Q>, env: Env) -> Self {
		MinimalEnvInfo {
			querier: Rc::new(deps.querier),
			env: Rc::new(env)
		}
	}
	pub fn from_deps_mut(deps: DepsMut<'exec, Q>, env: Env) -> Self {
		MinimalEnvInfo {
			querier: Rc::new(deps.querier),
			env: Rc::new(env)
		}
	}
}

#[deprecated(note = "please use `MinimalEnvInfo` instead. \"api\" and \"storage\" has been superseded by _not_ using it.")]
#[derive(Clone)]
pub struct ClonableEnvInfo<'exec, Q: CustomQuery = Empty> {
	pub storage: Rc<&'exec dyn Storage>,
	pub api: Rc<&'exec dyn Api>,
	pub querier: Rc<QuerierWrapper<'exec, Q>>,
	pub env: Rc<Env>,
}
#[allow(deprecated)]
impl<'exec, Q: CustomQuery> ClonableEnvInfo<'exec, Q> {
	pub fn new(deps: Deps<'exec, Q>, env: Env) -> Self {
		ClonableEnvInfo {
			storage: Rc::new(deps.storage),
			api: Rc::new(deps.api),
			querier: Rc::new(deps.querier),
			env: Rc::new(env),
		}
	}
}

#[deprecated(note = "please use `MinimalEnvInfo` instead. \"api\" and \"storage\" has been superseded by _not_ using it.")]
#[derive(Clone)]
pub struct ClonableEnvInfoMut<'exec, Q: CustomQuery = Empty> {
	pub storage: Rc<RefCell<&'exec mut dyn Storage>>,
	pub api: Rc<&'exec dyn Api>,
	pub querier: Rc<QuerierWrapper<'exec, Q>>,
	pub env: Rc<Env>,
}
#[allow(deprecated)]
impl<'exec, Q: CustomQuery> ClonableEnvInfoMut<'exec, Q> {
	pub fn new(deps: DepsMut<'exec, Q>, env: Env) -> Self {
		ClonableEnvInfoMut {
			storage: Rc::new(RefCell::new(deps.storage)),
			api: Rc::new(deps.api),
			querier: Rc::new(deps.querier),
			env: Rc::new(env),
		}
	}
}
