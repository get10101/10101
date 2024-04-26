use dlc_manager::contract::Contract;
use dlc_manager::ContractId;
use serde::Serialize;
use serde::Serializer;

#[derive(Serialize, Debug)]
pub struct ContractDetails {
    #[serde(serialize_with = "contract_id_as_hex")]
    pub contract_id: ContractId,
    #[serde(serialize_with = "contract_id_as_hex")]
    pub temporary_contract_id: ContractId,
    pub contract_state: ContractState,
    pub offered_funding_sats: Option<u64>,
    pub accepted_funding_sats: Option<u64>,
    pub fee_rate_per_vb: Option<u64>,
    pub event_id: Option<String>,
}

#[derive(Serialize, Debug)]
pub enum ContractState {
    Offered,
    Accepted,
    Signed,
    Confirmed,
    PreClosed,
    Closed,
    Refunded,
    FailedAccept,
    FailedSign,
    Rejected,
}

impl From<Contract> for ContractDetails {
    fn from(contract: Contract) -> Self {
        let (
            contract_state,
            offered_funding_sats,
            accepted_funding_sats,
            fee_rate_per_vb,
            event_id,
        ) = match &contract {
            Contract::Offered(offered_contract) => (
                ContractState::Offered,
                Some(offered_contract.offer_params.collateral),
                None,
                Some(offered_contract.fee_rate_per_vb),
                offered_contract.contract_info.first().map(|ci| {
                    ci.oracle_announcements
                        .first()
                        .map(|oa| oa.oracle_event.event_id.clone())
                }),
            ),
            Contract::Accepted(accepted_contract) => {
                let offered_contract = accepted_contract.clone().offered_contract;
                (
                    ContractState::Accepted,
                    Some(offered_contract.offer_params.collateral),
                    Some(accepted_contract.accept_params.collateral),
                    Some(offered_contract.fee_rate_per_vb),
                    offered_contract.contract_info.first().map(|ci| {
                        ci.oracle_announcements
                            .first()
                            .map(|oa| oa.oracle_event.event_id.clone())
                    }),
                )
            }
            Contract::Signed(signed_contract) => {
                let accepted_contract = signed_contract.clone().accepted_contract;
                let offered_contract = accepted_contract.clone().offered_contract;
                (
                    ContractState::Signed,
                    Some(offered_contract.offer_params.collateral),
                    Some(accepted_contract.accept_params.collateral),
                    Some(offered_contract.fee_rate_per_vb),
                    offered_contract.contract_info.first().map(|ci| {
                        ci.oracle_announcements
                            .first()
                            .map(|oa| oa.oracle_event.event_id.clone())
                    }),
                )
            }
            Contract::Confirmed(confirmed_contract) => {
                let accepted_contract = confirmed_contract.clone().accepted_contract;
                let offered_contract = accepted_contract.clone().offered_contract;
                (
                    ContractState::Confirmed,
                    Some(offered_contract.offer_params.collateral),
                    Some(accepted_contract.accept_params.collateral),
                    Some(offered_contract.fee_rate_per_vb),
                    offered_contract.contract_info.first().map(|ci| {
                        ci.oracle_announcements
                            .first()
                            .map(|oa| oa.oracle_event.event_id.clone())
                    }),
                )
            }
            Contract::PreClosed(pre_closed_contract) => {
                let accepted_contract = pre_closed_contract
                    .signed_contract
                    .clone()
                    .accepted_contract;
                let offered_contract = accepted_contract.clone().offered_contract;
                (
                    ContractState::PreClosed,
                    Some(offered_contract.offer_params.collateral),
                    Some(accepted_contract.accept_params.collateral),
                    Some(offered_contract.fee_rate_per_vb),
                    offered_contract.contract_info.first().map(|ci| {
                        ci.oracle_announcements
                            .first()
                            .map(|oa| oa.oracle_event.event_id.clone())
                    }),
                )
            }
            Contract::Closed(_closed_contract) => (ContractState::Closed, None, None, None, None),
            Contract::Refunded(refunded_contract) => {
                let accepted_contract = refunded_contract.clone().accepted_contract;
                let offered_contract = accepted_contract.clone().offered_contract;
                (
                    ContractState::Refunded,
                    Some(offered_contract.offer_params.collateral),
                    Some(accepted_contract.accept_params.collateral),
                    Some(offered_contract.fee_rate_per_vb),
                    offered_contract.contract_info.first().map(|ci| {
                        ci.oracle_announcements
                            .first()
                            .map(|oa| oa.oracle_event.event_id.clone())
                    }),
                )
            }
            Contract::FailedAccept(failed_accept_contract) => {
                let offered_contract = failed_accept_contract.clone().offered_contract;
                (
                    ContractState::FailedAccept,
                    Some(offered_contract.offer_params.collateral),
                    None,
                    Some(offered_contract.fee_rate_per_vb),
                    offered_contract.contract_info.first().map(|ci| {
                        ci.oracle_announcements
                            .first()
                            .map(|oa| oa.oracle_event.event_id.clone())
                    }),
                )
            }
            Contract::FailedSign(failed_sign_contract) => {
                let accepted_contract = failed_sign_contract.clone().accepted_contract;
                let offered_contract = accepted_contract.clone().offered_contract;
                (
                    ContractState::FailedSign,
                    Some(offered_contract.offer_params.collateral),
                    Some(accepted_contract.accept_params.collateral),
                    Some(offered_contract.fee_rate_per_vb),
                    offered_contract.contract_info.first().map(|ci| {
                        ci.oracle_announcements
                            .first()
                            .map(|oa| oa.oracle_event.event_id.clone())
                    }),
                )
            }
            Contract::Rejected(rejected_contract) => (
                ContractState::Rejected,
                Some(rejected_contract.offer_params.collateral),
                None,
                Some(rejected_contract.fee_rate_per_vb),
                rejected_contract.contract_info.first().map(|ci| {
                    ci.oracle_announcements
                        .first()
                        .map(|oa| oa.oracle_event.event_id.clone())
                }),
            ),
        };

        ContractDetails {
            contract_id: contract.get_id(),
            temporary_contract_id: contract.get_temporary_id(),
            contract_state,
            offered_funding_sats,
            accepted_funding_sats,
            fee_rate_per_vb,
            event_id: event_id.flatten(),
        }
    }
}

fn contract_id_as_hex<S>(contract_id: &ContractId, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&hex::encode(contract_id))
}
