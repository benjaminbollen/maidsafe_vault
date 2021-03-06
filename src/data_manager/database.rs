// Copyright 2015 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

use cbor;
use rustc_serialize::Encodable;
use std::collections::HashMap;

use transfer_parser::transfer_tags::DATA_MANAGER_ACCOUNT_TAG;

type PmidNode = ::routing::NameType;

pub type DataName = ::routing::NameType;
pub type PmidNodes = Vec<PmidNode>;

#[derive(RustcEncodable, RustcDecodable, PartialEq, Eq, Debug, Clone)]
pub struct Account {
    name: DataName,
    data_holders: PmidNodes,
    preserialised_content: Vec<u8>,
    has_preserialised_content: bool,
}

impl Account {
    pub fn new(name: DataName, data_holders: PmidNodes) -> Account {
        Account {
            name: name,
            data_holders: data_holders,
            preserialised_content: Vec::new(),
            has_preserialised_content: false,
        }
    }

    pub fn name(&self) -> &DataName {
        &self.name
    }

    pub fn data_holders(&self) -> &PmidNodes {
        &self.data_holders
    }
}

impl ::types::Refreshable for Account {
    fn serialised_contents(&self) -> Vec<u8> {
        if self.has_preserialised_content {
            self.preserialised_content.clone()
        } else {
            ::routing::utils::encode(&self).unwrap_or(vec![])
        }
    }

    fn merge(from_group: ::routing::NameType, responses: Vec<Account>) -> Option<Account> {
        let mut stats = Vec::<(PmidNodes, u64)>::new();
        for response in responses {
            let account =
                match ::routing::utils::decode::<Account>(&response.serialised_contents()) {
                    Ok(result) => {
                        if *result.name() != from_group {
                            continue;
                        }
                        result
                    }
                    Err(_) => continue,
                };
            let push_in_vec = match stats.iter_mut().find(|a| a.0 == *account.data_holders()) {
                Some(find_res) => {
                    find_res.1 += 1;
                    false
                }
                None => {
                    true
                }
            };
            if push_in_vec {
                stats.push((account.data_holders().clone(), 1));
            }
        }
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        let (pmids, count) = stats[0].clone();
        if count >= (::routing::types::GROUP_SIZE as u64 + 1) / 2 {
            return Some(Account::new(from_group, pmids));
        }
        None
    }
}



pub struct Database {
    storage: HashMap<DataName, PmidNodes>,
    pub close_grp_from_churn: Vec<::routing::NameType>,
    pub temp_storage_after_churn: HashMap<::routing::NameType, PmidNodes>,
}

impl Database {
    pub fn new() -> Database {
        Database {
            storage: HashMap::with_capacity(10000),
            close_grp_from_churn: Vec::new(),
            temp_storage_after_churn: HashMap::new(),
        }
    }

    pub fn exist(&mut self, name: &DataName) -> bool {
        self.storage.contains_key(name)
    }

    pub fn put_pmid_nodes(&mut self, name: &DataName, pmid_nodes: PmidNodes) {
        let _ = self.storage.entry(name.clone()).or_insert(pmid_nodes.clone());
    }

    pub fn add_pmid_node(&mut self, name: &DataName, pmid_node: PmidNode) {
        let nodes = self.storage.entry(name.clone()).or_insert(vec![pmid_node.clone()]);
        if !nodes.contains(&pmid_node) {
            nodes.push(pmid_node);
        }
    }

    pub fn remove_pmid_node(&mut self, name: &DataName, pmid_node: PmidNode) {
        if !self.storage.contains_key(name) {
            return;
        }
        let nodes = self.storage.entry(name.clone()).or_insert(vec![]);
        for i in 0..nodes.len() {
            if nodes[i] == pmid_node {
                let _ = nodes.remove(i);
                break;
            }
        }
    }

    pub fn get_pmid_nodes(&mut self, name: &DataName) -> PmidNodes {
        match self.storage.get(&name) {
            Some(entry) => entry.clone(),
            None => Vec::<PmidNode>::new(),
        }
    }


    pub fn handle_account_transfer(&mut self, merged_account: Account) {
        let _ = self.storage.remove(merged_account.name());
        let _ = self.storage.insert(*merged_account.name(), merged_account.data_holders().clone());
        info!("DataManager updated account {:?} to {:?}",
              merged_account.name(), merged_account.data_holders());
    }

    pub fn retrieve_all_and_reset(&mut self,
                                  _close_group: &mut Vec<::routing::NameType>)
                                  -> Vec<::types::MethodCall> {
        self.temp_storage_after_churn = self.storage.clone();
        let mut actions = Vec::<::types::MethodCall>::new();
        for (key, value) in self.storage.iter() {
            if value.len() < 3 {
                for pmid_node in value.iter() {
                    info!("DataManager sends out a Get request in churn, fetching data {:?} from \
                          pmid_node {:?}", *key, pmid_node);
                    actions.push(::types::MethodCall::Get {
                        location: ::routing::authority::Authority::ManagedNode(pmid_node.clone()),
                        // DataManager only handles ::routing::immutable_data::ImmutableData
                        data_request:
                            ::routing::data::DataRequest::ImmutableData((*key).clone(),
                                ::routing::immutable_data::ImmutableDataType::Normal)
                    });
                }
            }
            let account = Account::new((*key).clone(), (*value).clone());
            let mut encoder = cbor::Encoder::from_memory();
            if encoder.encode(&[account.clone()]).is_ok() {
                debug!("DataManager sends out a refresh regarding account {:?}", account.name());
                actions.push(::types::MethodCall::Refresh {
                    type_tag: DATA_MANAGER_ACCOUNT_TAG,
                    our_authority: ::routing::Authority::NaeManager(*account.name()),
                    payload: encoder.as_bytes().to_vec()
                });
            }
        }
        self.storage.clear();
        debug!("DataManager storage cleaned in churn with actions.len() = {:?}", actions.len());
        actions
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn exist() {
        let mut db = Database::new();
        let value = ::routing::types::generate_random_vec_u8(1024);
        let data = ::routing::immutable_data::ImmutableData::new(
                       ::routing::immutable_data::ImmutableDataType::Normal, value);
        let mut pmid_nodes: Vec<::routing::NameType> = vec![];

        for _ in 0..4 {
            pmid_nodes.push(::utils::random_name());
        }

        let data_name = data.name();
        assert_eq!(db.exist(&data_name), false);
        db.put_pmid_nodes(&data_name, pmid_nodes);
        assert_eq!(db.exist(&data_name), true);
    }

    #[test]
    fn put() {
        let mut db = Database::new();
        let value = ::routing::types::generate_random_vec_u8(1024);
        let data = ::routing::immutable_data::ImmutableData::new(
                       ::routing::immutable_data::ImmutableDataType::Normal, value);
        let data_name = data.name();
        let mut pmid_nodes: Vec<::routing::NameType> = vec![];

        for _ in 0..4 {
            pmid_nodes.push(::utils::random_name());
        }

        let result = db.get_pmid_nodes(&data_name);
        assert_eq!(result.len(), 0);

        db.put_pmid_nodes(&data_name, pmid_nodes.clone());

        let result = db.get_pmid_nodes(&data_name);
        assert_eq!(result.len(), pmid_nodes.len());
    }

    #[test]
    fn remove_pmid() {
        let mut db = Database::new();
        let value = ::routing::types::generate_random_vec_u8(1024);
        let data = ::routing::immutable_data::ImmutableData::new(
                       ::routing::immutable_data::ImmutableDataType::Normal, value);
        let data_name = data.name();
        let mut pmid_nodes: Vec<::routing::NameType> = vec![];

        for _ in 0..4 {
            pmid_nodes.push(::utils::random_name());
        }

        db.put_pmid_nodes(&data_name, pmid_nodes.clone());
        let result = db.get_pmid_nodes(&data_name);
        assert_eq!(result, pmid_nodes);

        db.remove_pmid_node(&data_name, pmid_nodes[0].clone());

        let result = db.get_pmid_nodes(&data_name);
        assert_eq!(result.len(), 3);
        for index in 0..result.len() {
            assert!(result[index] != pmid_nodes[0]);
        }
    }

    #[test]
    fn replace_pmids() {
        let mut db = Database::new();
        let value = ::routing::types::generate_random_vec_u8(1024);
        let data = ::routing::immutable_data::ImmutableData::new(
                       ::routing::immutable_data::ImmutableDataType::Normal, value);
        let data_name = data.name();
        let mut pmid_nodes: Vec<::routing::NameType> = vec![];
        let mut new_pmid_nodes: Vec<::routing::NameType> = vec![];

        for _ in 0..4 {
            pmid_nodes.push(::utils::random_name());
            new_pmid_nodes.push(::utils::random_name());
        }

        db.put_pmid_nodes(&data_name, pmid_nodes.clone());
        let result = db.get_pmid_nodes(&data_name);
        assert_eq!(result, pmid_nodes);
        assert!(result != new_pmid_nodes);

        for index in 0..4 {
            db.remove_pmid_node(&data_name, pmid_nodes[index].clone());
            db.add_pmid_node(&data_name, new_pmid_nodes[index].clone());
        }

        let result = db.get_pmid_nodes(&data_name);
        assert_eq!(result, new_pmid_nodes);
        assert!(result != pmid_nodes);
    }

    #[test]
    fn handle_account_transfer() {
        let mut db = Database::new();
        let value = ::routing::types::generate_random_vec_u8(1024);
        let data = ::routing::immutable_data::ImmutableData::new(
                       ::routing::immutable_data::ImmutableDataType::Normal, value);
        let data_name = data.name();
        let mut pmid_nodes: Vec<::routing::NameType> = vec![];

        for _ in 0..4 {
            pmid_nodes.push(::utils::random_name());
        }
        db.put_pmid_nodes(&data_name, pmid_nodes.clone());
        assert_eq!(db.get_pmid_nodes(&data_name).len(), pmid_nodes.len());

        db.handle_account_transfer(Account::new(data_name.clone(), vec![]));
        assert_eq!(db.get_pmid_nodes(&data_name).len(), 0);
    }
}
