#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod voting {
    use ink_prelude::vec::Vec;
    use ink_storage::{
        traits::PackedLayout, traits::SpreadAllocate, traits::SpreadLayout, Mapping,
    };

    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct Voting {
        elections: Mapping<u32, (AccountId, bool, RegistrationState, ElectionState)>,
        elections_list: Vec<Vec<u8>>,
        elections_ids: Mapping<Vec<u8>, u32>,
        vote_proposals: Mapping<(u32, u32), u128>,
        proposals_ids: Mapping<(u32, Vec<u8>), u32>,
        proposals_list: Mapping<u32, Vec<Vec<u8>>>,
        voters: Mapping<(u32, AccountId), (u128, bool)>,
        election_nonce: u32,
        election_count: u32,
    }

    #[ink(event)]
    pub struct ElectionCreated {
        name: Vec<u8>,
        id: u32,
        owner: AccountId,
        require_registration: bool,
    }

    #[ink(event)]
    pub struct Voted {
        voter: AccountId,
        proposal: Vec<u8>,
        weight: u128,
    }

    #[ink(event)]
    pub struct Registered {
        voter: AccountId,
        election_id: u32,
    }

    #[ink(event)]
    pub struct OpenRegistration {
        election_id: u32,
        date: Timestamp,
    }

    #[ink(event)]
    pub struct CloseRegistration {
        election_id: u32,
        date: Timestamp,
    }

    #[ink(event)]
    pub struct OpenElection {
        election_id: u32,
        date: Timestamp,
    }

    #[ink(event)]
    pub struct CloseElection {
        election_id: u32,
        date: Timestamp,
    }

    #[ink(event)]
    pub struct ChangeOwnership {
        election_id: u32,
        new_owner: AccountId,
    }

    #[ink(event)]
    pub struct Delegate {
        election_id: u32,
        delegate: AccountId,
        delegator: AccountId,
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        InsufficientProposals,
        ElectionNotValid,
        VoterNotRegistred,
        VoterHasNotSoMuchWeight,
        VoterHasAlreadyVoted,
        InvalidProposal,
        OnlyOwner,
        ElectionClosed,
        RegistrationClosed,
        VoterAlreadyRegistered,
    }

    #[derive(SpreadLayout, PackedLayout, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum RegistrationState {
        RegistrationOpen,
        RegistrationClosed,
    }
    impl Default for RegistrationState {
        fn default() -> Self {
            RegistrationState::RegistrationClosed
        }
    }

    #[derive(SpreadLayout, PackedLayout, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ElectionState {
        ElectionOpen,
        ElectionClosed,
    }

    impl Default for ElectionState {
        fn default() -> Self {
            ElectionState::ElectionClosed
        }
    }

    pub type Result<T> = core::result::Result<T, Error>;

    impl Voting {
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|contract| Self::new_init(contract))
        }

        fn new_init(&mut self) {
            self.election_nonce = 1;
            self.election_count = 0;
        }

        #[ink(message)]
        pub fn create_election(
            &mut self,
            name: Vec<u8>,
            required_registration: bool,
            proposals: Vec<Vec<u8>>,
        ) -> Result<()> {
            self.check_double_election(&name)?;
            self.check_sufficient_proposals(&proposals)?;
            let election_id = self.election_nonce;
            self.insert_election(
                &name,
                &election_id,
                Self::env().caller(),
                required_registration,
                &proposals,
            );
            Self::env().emit_event(ElectionCreated {
                name: name,
                id: election_id,
                owner: Self::env().caller(),
                require_registration: required_registration,
            });
            self.election_nonce += 1;
            self.election_count += 1;
            Ok(())
        }
        
        #[ink(message)]
        pub fn vote(&mut self, election_id: u32, proposal: Vec<u8>, weight: u128) -> Result<()> {
            self.check_id_existence(&election_id)?;
            self.check_election_open(&election_id)?;
            let voter_address = Self::env().caller();
            self.check_if_registration_needed(&election_id, &voter_address)?;
            self.check_voter_can_vote(&election_id, &voter_address, &weight)?;
            self.check_proposal_valid(&election_id, &proposal)?;
            self._vote(&election_id, &proposal, &voter_address, &weight);
            Self::env().emit_event(Voted {
                voter: voter_address,
                proposal: proposal,
                weight: weight,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn register_me(&mut self, election_id: u32) -> Result<()> {
            self.register(election_id, Self::env().caller())?;
            Ok(())
        }

        #[ink(message)]
        pub fn register(&mut self, election_id: u32, voter: AccountId) -> Result<()> {
            self.check_id_existence(&election_id)?;
            self.check_registration_open(&election_id)?;
            if self.is_voter_registered(&election_id, &voter) {
                return Err(Error::VoterAlreadyRegistered);
            };
            self.register_voter(&voter, &election_id);
            Self::env().emit_event(Registered {
                voter: voter,
                election_id: election_id,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn delegate_vote(
            &mut self,
            election_id: u32,
            delegate: AccountId,
            weight: u128,
        ) -> Result<()> {
            self.check_id_existence(&election_id)?;
            let delegator = Self::env().caller();
            self.check_if_registration_needed(&election_id, &delegator)?;
            self.check_if_registration_needed(&election_id, &delegate)?;
            self.check_voter_can_vote(&election_id, &delegator, &weight)?;
            self.delegate(&election_id, &delegate, &delegator, &weight);
            Self::env().emit_event(Delegate {
                election_id: election_id,
                delegate: delegate,
                delegator: delegator,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn open_registration(&mut self, election_id: u32) -> Result<()> {
            self.check_id_existence(&election_id)?;
            self.only_owner(&election_id, &Self::env().caller())?;
            let mut election = self.elections.get(election_id).unwrap();
            election.2 = RegistrationState::RegistrationOpen;
            self.elections.insert(election_id, &election);
            Self::env().emit_event(OpenRegistration {
                election_id: election_id,
                date: Self::env().block_timestamp(),
            });
            Ok(())
        }

        #[ink(message)]
        pub fn close_registration(&mut self, election_id: u32) -> Result<()> {
            self.check_id_existence(&election_id)?;
            self.only_owner(&election_id, &Self::env().caller())?;
            let mut election = self.elections.get(election_id).unwrap();
            election.2 = RegistrationState::RegistrationClosed;
            self.elections.insert(election_id, &election);
            Self::env().emit_event(CloseRegistration {
                election_id: election_id,
                date: Self::env().block_timestamp(),
            });
            Ok(())
        }

        #[ink(message)]
        pub fn open_election(&mut self, election_id: u32) -> Result<()> {
            self.check_id_existence(&election_id)?;
            self.only_owner(&election_id, &Self::env().caller())?;
            let mut election = self.elections.get(election_id).unwrap();
            election.3 = ElectionState::ElectionOpen;
            self.elections.insert(election_id, &election);
            Self::env().emit_event(OpenElection {
                election_id: election_id,
                date: Self::env().block_timestamp(),
            });
            Ok(())
        }

        #[ink(message)]
        pub fn close_election(&mut self, election_id: u32) -> Result<()> {
            self.check_id_existence(&election_id)?;
            self.only_owner(&election_id, &Self::env().caller())?;
            let mut election = self.elections.get(election_id).unwrap();
            election.3 = ElectionState::ElectionClosed;
            self.elections.insert(election_id, &election);
            Self::env().emit_event(CloseElection {
                election_id: election_id,
                date: Self::env().block_timestamp(),
            });
            Ok(())
        }

        #[ink(message)]
        pub fn change_ownership(&mut self, election_id: u32, new_owner: AccountId) -> Result<()> {
            self.check_id_existence(&election_id)?;
            self.only_owner(&election_id, &Self::env().caller())?;
            let mut election = self.elections.get(election_id).unwrap();
            election.0 = new_owner;
            self.elections.insert(election_id, &election);
            Self::env().emit_event(ChangeOwnership {
                election_id: election_id,
                new_owner: new_owner,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn get_number_elections(&self) -> u32 {
            self.election_count
        }

        #[ink(message)]
        pub fn get_election_id(&self, name: Vec<u8>) -> u32 {
            self.elections_ids.get(name).unwrap_or_default()
        }

        #[ink(message)]
        pub fn get_election_list(&self) -> Vec<Vec<u8>> {
            self.elections_list.clone()
        }

        #[ink(message)]
        pub fn get_proposal_for_election(&self, name: Vec<u8>) -> Vec<Vec<u8>> {
            let election_id = self.elections_ids.get(&name).unwrap_or_default();
            self.proposals_list.get(election_id).unwrap_or_default()
        }

        #[ink(message)]
        pub fn get_owner_of_election(&self, election_id: u32) -> AccountId {
            self.elections.get(election_id).unwrap_or_default().0
        }

        #[ink(message)]
        pub fn election_exists(&self, name: Vec<u8>) -> bool {
            self._election_name_exists(&name)
        }
        #[ink(message)]
        pub fn is_election_open(&self, election_id: u32) -> bool {
            self._is_election_open(&election_id)
        }
        #[ink(message)]
        pub fn is_registration_open(&self, election_id: u32) -> bool {
            self._is_registration_open(&election_id)
        }
        #[ink(message)]
        pub fn is_account_registered(&self, election_id: u32, account: AccountId) -> bool {
            self.voters.get((election_id, account)) != None
        }
        
        #[ink(message)]
        pub fn get_result_election(&self, election_id: u32) -> Vec<(Vec<u8>, u128)> {
            let mut result = Vec::new();
            for proposal in self.proposals_list.get(election_id).unwrap_or_default() {
                let proposal_id = self
                    .proposals_ids
                    .get((election_id, &proposal))
                    .unwrap_or_default();
                result.push((
                    proposal,
                    self.vote_proposals
                        .get((election_id, proposal_id))
                        .unwrap_or_default(),
                ));
            }
            result
        }
        #[ink(message)]
        pub fn get_votes_proposal(&self, election_id: u32, proposal: Vec<u8>) -> u128 {
            let proposal_id = self
                .proposals_ids
                .get((election_id, proposal))
                .unwrap_or_default();
            self.vote_proposals
                .get((election_id, proposal_id))
                .unwrap_or_default()
        }

        #[ink(message)]
        pub fn get_winner(&self, election_id: u32) -> (Vec<u8>, u128) {
            let mut winner = Vec::new();
            let mut max_votes = 0;
            let mut proposal_id;
            let mut vote_proposal;
            for proposal in self.proposals_list.get(election_id).unwrap_or_default() {
                proposal_id = self
                    .proposals_ids
                    .get((election_id, &proposal))
                    .unwrap_or_default();
                vote_proposal = self
                    .vote_proposals
                    .get((election_id, proposal_id))
                    .unwrap_or_default();
                if vote_proposal > max_votes {
                    winner = proposal;
                    max_votes = self
                        .vote_proposals
                        .get((election_id, proposal_id))
                        .unwrap_or_default()
                }
            }
            (winner, max_votes)
        }

        #[ink(message)]
        pub fn get_voter_weigth(&self, election_id: u32, voter: AccountId) -> u128 {
            self.voters.get((election_id, voter)).unwrap_or_default().0
        }

        #[ink(message)]
        pub fn has_voter_voted(&self, election_id: u32, voter: AccountId) -> bool {
            self.voters.get((election_id, voter)).unwrap_or_default().1
        }

        fn subtract_weight(&mut self, election_id: &u32, voter: &AccountId, weight: &u128) {
            let voter_weight = self.voters.get((election_id, voter)).unwrap().0;
            if voter_weight - weight == 0 {
                self.voters.insert((&election_id, &voter), &(0, true));
            } else {
                self.voters
                    .insert((&election_id, &voter), &(voter_weight - weight, false));
            }
        }

        fn register_voter(&mut self, voter: &AccountId, election_id: &u32) {
            self.voters.insert((election_id, voter), &(1, false));
        }
        
        fn _election_name_exists(&self, name: &Vec<u8>) -> bool {
            self.elections_ids.get(name) != None
        }

        fn _is_election_open(&self, election_id: &u32) -> bool {
            self.elections.get(election_id).unwrap_or_default().3 == ElectionState::ElectionOpen
        }

        fn _is_registration_open(&self, election_id: &u32) -> bool {
            self.elections.get(election_id).unwrap_or_default().2
                == RegistrationState::RegistrationOpen
        }

        fn delegate(
            &mut self,
            election_id: &u32,
            delegate: &AccountId,
            delegator_address: &AccountId,
            weight: &u128,
        ) {
            let weight_delegate = &self.voters.get((election_id, delegate)).unwrap().0;
            self.voters
                .insert((election_id, delegate), &(weight_delegate + weight, false));
            self.subtract_weight(election_id, delegator_address, weight);
        }

        fn is_owner(&self, account: &AccountId, election_id: &u32) -> bool {
            account == &self.elections.get(election_id).unwrap_or_default().0
        }
        fn check_id_existence(&self, id: &u32) -> Result<()> {
            if !self._election_id_exists(id) {
                Err(Error::ElectionNotValid)
            } else {
                Ok(())
            }
        }
        fn _election_id_exists(&self, id: &u32) -> bool {
            self.elections.get(id) != None
        }
        fn check_sufficient_proposals(&self, proposals: &Vec<Vec<u8>>) -> Result<()> {
            if proposals.len() == 0 {
                Err(Error::InsufficientProposals)
            } else {
                Ok(())
            }
        }
        fn insert_proposal(&mut self, election_id: &u32, proposal: &Vec<u8>, proposal_id: &u32) {
            self.vote_proposals
                .insert((election_id, (proposal_id + 1)), &0);
            self.proposals_ids
                .insert((election_id, proposal), &(proposal_id + 1));
        }
        fn insert_election(
            &mut self,
            name: &Vec<u8>,
            election_id: &u32,
            owner: AccountId,
            required_registration: bool,
            proposals: &Vec<Vec<u8>>,
        ) {
            self.elections.insert(
                &election_id,
                &(
                    owner,
                    required_registration,
                    RegistrationState::RegistrationClosed,
                    ElectionState::ElectionClosed,
                ),
            );
            self.elections_ids.insert(name, election_id);
            self.elections_list.push(name.to_vec());
            for i in 0..proposals.len() {
                self.insert_proposal(&election_id, proposals.get(i).unwrap(), &(i as u32));
            }
            self.proposals_list.insert(&election_id, proposals);
        }
        fn check_election_open(&self, election_id: &u32) -> Result<()> {
            if !self._is_election_open(election_id) {
                Err(Error::ElectionClosed)
            } else {
                Ok(())
            }
        }
        fn check_registration_open(&self, election_id: &u32) -> Result<()> {
            if !self._is_registration_open(election_id) {
                Err(Error::RegistrationClosed)
            } else {
                Ok(())
            }
        }
        fn is_voter_registered(&self, election_id: &u32, voter_address: &AccountId) -> bool {
            !(self.voters.get((election_id, voter_address)) == None)
        }

        fn check_voter_registered(
            &self,
            election_id: &u32,
            voter_address: &AccountId,
        ) -> Result<()> {
            if !self.is_voter_registered(election_id, voter_address) {
                Err(Error::VoterNotRegistred)
            } else {
                Ok(())
            }
        }
        fn check_proposal_valid(&self, election_id: &u32, proposal: &Vec<u8>) -> Result<()> {
            if self.proposals_ids.get((election_id, proposal)) == None {
                Err(Error::InvalidProposal)
            } else {
                Ok(())
            }
        }
        fn check_voter_can_vote(
            &self,
            election_id: &u32,
            voter_address: &AccountId,
            weight: &u128,
        ) -> Result<()> {
            let (voter_weight, voter_has_voted) =
                self.voters.get((election_id, voter_address)).unwrap();
            if voter_has_voted {
                Err(Error::VoterHasAlreadyVoted)
            } else if &voter_weight < weight {
                Err(Error::VoterHasNotSoMuchWeight)
            } else {
                Ok(())
            }
        }
        fn _vote(
            &mut self,
            election_id: &u32,
            proposal: &Vec<u8>,
            voter_address: &AccountId,
            weight: &u128,
        ) {
            let proposal_id = self.proposals_ids.get((election_id, proposal)).unwrap();
            let vote_proposal = self.vote_proposals.get((election_id, proposal_id)).unwrap();
            self.vote_proposals
                .insert((election_id, proposal_id), &(vote_proposal + weight));
            self.subtract_weight(election_id, voter_address, weight);
        }
        fn only_owner(&self, election_id: &u32, address: &AccountId) -> Result<()> {
            if !self.is_owner(address, election_id) {
                Err(Error::OnlyOwner)
            } else {
                Ok(())
            }
        }
        fn check_if_registration_needed(
            &mut self,
            election_id: &u32,
            voter_address: &AccountId,
        ) -> Result<()> {
            if self.elections.get(&election_id).unwrap().1 {
                self.check_voter_registered(&election_id, &voter_address)?;
            } else {
                if !self.is_voter_registered(&election_id, &voter_address) {
                    self.register_voter(&voter_address, &election_id);
                }
            }
            Ok(())
        }
        fn check_double_election(&self, name: &Vec<u8>) -> Result<()> {
            if self._election_name_exists(name) {
                Err(Error::ElectionNotValid)
            } else {
                Ok(())
            }
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// Imports `ink_lang` so we can use `#[ink::test]`.
        use ink_lang as ink;

        /// We test if the default constructor does its job.
        #[ink::test]
        fn new_works() {
            let voting = Voting::new();
            assert_eq!(voting.get_number_elections(), 0);
        }

        fn to_ut8(string: &str) -> Vec<u8> {
            string.as_bytes().to_vec()
        }
        fn initialize_and_create_election(require_registration: bool) -> Result<Voting> {
            let mut voting = Voting::new();
            voting.create_election(
                to_ut8("firstelection"),
                require_registration,
                vec![to_ut8("firstproposal"), to_ut8("secondproposal")],
            )?;
            Ok(voting)
        }

        /// We test a simple use case of our contract.
        #[ink::test]
        fn create_election_works() {
            let mut voting = Voting::new();
            //let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();
            assert_eq!(
                voting.create_election(
                    to_ut8("firstelection"),
                    false,
                    vec![to_ut8("firstproposal"), to_ut8("secondproposal")]
                ),
                Ok(())
            );
            assert_eq!(
                voting.create_election(
                    to_ut8("firstelection"),
                    false,
                    vec![to_ut8("firstproposal")]
                ),
                Err(Error::ElectionNotValid)
            );
            assert_eq!(
                voting.create_election(to_ut8("secondelection"), false, vec![]),
                Err(Error::InsufficientProposals)
            );
            assert_eq!(voting.get_number_elections(), 1);
            assert_eq!(ink_env::test::recorded_events().count(), 1);
            assert_eq!(
                voting.get_proposal_for_election(to_ut8("firstelection")),
                vec![to_ut8("firstproposal"), to_ut8("secondproposal")]
            );
            assert_eq!(
                voting.get_owner_of_election(1),
                ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().alice
            );
        }
        #[ink::test]
        fn open_election_works() {
            let mut voting = initialize_and_create_election(false).unwrap();
            assert_eq!(voting.is_election_open(1), false);
            assert_eq!(voting.open_election(1), Ok(()));
            assert_eq!(voting.is_election_open(1), true);
            assert_eq!(
                voting.open_election(2),
                Err(Error::ElectionNotValid)
            );
            let bob = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().bob;
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(bob);
            assert_eq!(voting.open_election(1), Err(Error::OnlyOwner));
            assert_eq!(ink_env::test::recorded_events().count(), 2);
        }
        #[ink::test]
        fn close_election_works() {
            let mut voting = initialize_and_create_election(false).unwrap();
            voting.open_election(1).unwrap();
            assert_eq!(voting.is_election_open(1), true);
            assert_eq!(voting.close_election(1), Ok(()));
            assert_eq!(voting.is_election_open(1), false);
            assert_eq!(
                voting.open_election(2),
                Err(Error::ElectionNotValid)
            );
            let bob = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().bob;
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(bob);
            assert_eq!(voting.close_election(1), Err(Error::OnlyOwner));
            assert_eq!(ink_env::test::recorded_events().count(), 3);
        }
        #[ink::test]
        fn open_registration_works() {
            let mut voting = initialize_and_create_election(false).unwrap();
            assert_eq!(voting.is_registration_open(1), false);
            assert_eq!(voting.open_registration(1), Ok(()));
            assert_eq!(voting.is_registration_open(1), true);
            assert_eq!(
                voting.open_registration(2),
                Err(Error::ElectionNotValid)
            );
            let bob = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().bob;
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(bob);
            assert_eq!(voting.open_registration(1), Err(Error::OnlyOwner));
            assert_eq!(ink_env::test::recorded_events().count(), 2);
        }
        #[ink::test]
        fn close_registration_works() {
            let mut voting = initialize_and_create_election(false).unwrap();
            voting.open_registration(1).unwrap();
            assert_eq!(voting.is_registration_open(1), true);
            assert_eq!(voting.close_registration(1), Ok(()));
            assert_eq!(voting.is_registration_open(1), false);
            assert_eq!(
                voting.open_registration(2),
                Err(Error::ElectionNotValid)
            );
            let bob = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().bob;
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(bob);
            assert_eq!(voting.close_registration(1), Err(Error::OnlyOwner));
            assert_eq!(ink_env::test::recorded_events().count(), 3);
        }
        #[ink::test]
        fn change_ownership_works() {
            let mut voting = initialize_and_create_election(false).unwrap();
            let bob = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().bob;
            assert_eq!(
                voting.change_ownership(2, bob),
                Err(Error::ElectionNotValid)
            );
            assert_eq!(voting.change_ownership(1, bob), Ok(()));
            assert_eq!(voting.get_owner_of_election(1), bob);
            assert_eq!(
                voting.change_ownership(1, bob),
                Err(Error::OnlyOwner)
            );
            assert_eq!(ink_env::test::recorded_events().count(), 2);
        }
        #[ink::test]
        fn register_works() {
            let mut voting = initialize_and_create_election(true).unwrap();
            let bob = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().bob;
            assert_eq!(
                voting.register(2, bob),
                Err(Error::ElectionNotValid)
            );
            assert_eq!(
                voting.register(1, bob),
                Err(Error::RegistrationClosed)
            );
            voting.open_registration(1).unwrap();
            assert_eq!(voting.register(1, bob), Ok(()));
            assert_eq!(voting.is_account_registered(1, bob), true);
            assert_eq!(voting.get_voter_weigth(1, bob), 1);
            assert_eq!(ink_env::test::recorded_events().count(), 3);
            assert_eq!(
                voting.register(1, bob),
                Err(Error::VoterAlreadyRegistered)
            );
        }
        #[ink::test]
        fn register_me_works() {
            let mut voting = initialize_and_create_election(true).unwrap();
            assert_eq!(voting.register_me(2), Err(Error::ElectionNotValid));
            assert_eq!(
                voting.register_me(1),
                Err(Error::RegistrationClosed)
            );
            voting.open_registration(1).unwrap();
            assert_eq!(voting.register_me(1), Ok(()));
            let alice = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().alice;
            assert_eq!(voting.is_account_registered(1, alice), true);
            assert_eq!(voting.get_voter_weigth(1, alice), 1);
            assert_eq!(ink_env::test::recorded_events().count(), 3);
            assert_eq!(
                voting.register_me(1),
                Err(Error::VoterAlreadyRegistered)
            );
        }
        #[ink::test]
        fn vote_without_registration_works() {
            let mut voting = initialize_and_create_election(false).unwrap();
            assert_eq!(
                voting.vote(2, to_ut8("firstproposal"), 1),
                Err(Error::ElectionNotValid)
            );
            assert_eq!(
                voting.vote(1, to_ut8("firstproposal"), 1),
                Err(Error::ElectionClosed)
            );
            voting.open_election(1).unwrap();
            assert_eq!(
                voting.vote(1, to_ut8("firstproposal"), 2),
                Err(Error::VoterHasNotSoMuchWeight)
            );
            assert_eq!(
                voting.vote(1, to_ut8("inexistentproposal"), 1),
                Err(Error::InvalidProposal)
            );
            assert_eq!(voting.vote(1, to_ut8("firstproposal"), 1), Ok(()));
            let alice = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().alice;
            assert_eq!(voting.has_voter_voted(1, alice), true);
            assert_eq!(voting.get_voter_weigth(1, alice), 0);
            assert_eq!(ink_env::test::recorded_events().count(), 3);
            assert_eq!(
                voting.get_votes_proposal(1, to_ut8("firstproposal")),
                1
            );
            assert_eq!(
                voting.vote(1, to_ut8("firstproposal"), 1),
                Err(Error::VoterHasAlreadyVoted)
            );
        }
        #[ink::test]
        fn vote_with_registration_works() {
            let mut voting = initialize_and_create_election(true).unwrap();
            assert_eq!(
                voting.vote(2, to_ut8("firstproposal"), 1),
                Err(Error::ElectionNotValid)
            );
            assert_eq!(
                voting.vote(1, to_ut8("firstproposal"), 1),
                Err(Error::ElectionClosed)
            );
            voting.open_election(1).unwrap();
            assert_eq!(
                voting.vote(1, to_ut8("firstproposal"), 1),
                Err(Error::VoterNotRegistred)
            );
            voting.open_registration(1).unwrap();
            voting.register_me(1).unwrap();
            assert_eq!(
                voting.vote(1, to_ut8("firstproposal"), 2),
                Err(Error::VoterHasNotSoMuchWeight)
            );
            assert_eq!(
                voting.vote(1, to_ut8("inexistentproposal"), 1),
                Err(Error::InvalidProposal)
            );
            assert_eq!(voting.vote(1, to_ut8("firstproposal"), 1), Ok(()));
            let alice = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().alice;
            assert_eq!(voting.has_voter_voted(1, alice), true);
            assert_eq!(voting.get_voter_weigth(1, alice), 0);
            assert_eq!(ink_env::test::recorded_events().count(), 5);
            assert_eq!(
                voting.get_votes_proposal(1, to_ut8("firstproposal")),
                1
            );
            assert_eq!(
                voting.vote(1, to_ut8("firstproposal"), 1),
                Err(Error::VoterHasAlreadyVoted)
            );
        }
        #[ink::test]
        fn delegate_vote_without_registration_works() {
            let mut voting = initialize_and_create_election(false).unwrap();
            let bob = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().bob;
            let alice = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().alice;
            assert_eq!(
                voting.delegate_vote(2, bob, 1),
                Err(Error::ElectionNotValid)
            );
            assert_eq!(
                voting.delegate_vote(1, bob, 2),
                Err(Error::VoterHasNotSoMuchWeight)
            );
            assert_eq!(voting.delegate_vote(1, bob, 1), Ok(()));
            assert_eq!(voting.has_voter_voted(1, alice), true);
            assert_eq!(voting.get_voter_weigth(1, alice), 0);
            assert_eq!(voting.has_voter_voted(1, bob), false);
            assert_eq!(voting.get_voter_weigth(1, bob), 2);
            assert_eq!(ink_env::test::recorded_events().count(), 2);
            assert_eq!(
                voting.delegate_vote(1, bob, 1),
                Err(Error::VoterHasAlreadyVoted)
            );
        }
        #[ink::test]
        fn delegate_vote_with_registration_works() {
            let mut voting = initialize_and_create_election(true).unwrap();
            let bob = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().bob;
            let alice = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().alice;
            assert_eq!(
                voting.delegate_vote(2, bob, 1),
                Err(Error::ElectionNotValid)
            );
            assert_eq!(
                voting.delegate_vote(1, bob, 1),
                Err(Error::VoterNotRegistred)
            );
            voting.open_registration(1).unwrap();
            voting.register_me(1).unwrap();
            assert_eq!(
                voting.delegate_vote(1, bob, 1),
                Err(Error::VoterNotRegistred)
            );
            voting.register(1, bob).unwrap();
            assert_eq!(
                voting.delegate_vote(1, bob, 2),
                Err(Error::VoterHasNotSoMuchWeight)
            );
            assert_eq!(voting.delegate_vote(1, bob, 1), Ok(()));
            assert_eq!(voting.has_voter_voted(1, alice), true);
            assert_eq!(voting.get_voter_weigth(1, alice), 0);
            assert_eq!(voting.has_voter_voted(1, bob), false);
            assert_eq!(voting.get_voter_weigth(1, bob), 2);
            assert_eq!(ink_env::test::recorded_events().count(), 5);
            assert_eq!(
                voting.delegate_vote(1, bob, 1),
                Err(Error::VoterHasAlreadyVoted)
            );
        }
    }
}