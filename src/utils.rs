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

/// Returns the median (rounded down to the nearest integral value) of `values` which can be
/// unsorted.  If `values` is empty, returns `0`.
pub fn median(mut values: Vec<u64>) -> u64 {
    match values.len() {
        0 => 0u64,
        1 => values[0],
        len if len % 2 == 0 => {
            values.sort();
            let lower_value = values[(len / 2) - 1];
            let upper_value = values[len / 2];
            (lower_value + upper_value) / 2
        }
        len => {
            values.sort();
            values[len / 2]
        }
    }
}

#[cfg(test)]
pub fn random_name() -> ::routing::NameType {
    // TODO - once Routing provides either a compile-time value for `NameType`'s length or exposes
    // `NameType::generate_random()` this should be used here.  Issue reported at
    // https://github.com/maidsafe/routing/issues/674
    ::routing::NameType(::routing::types::vector_as_u8_64_array(
        ::routing::types::generate_random_vec_u8(64)))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get_median() {
        assert_eq!(0, median(vec![0u64; 0]));
        assert_eq!(9, median(vec![9]));
        assert_eq!(0, median(vec![1, 0]));
        assert_eq!(1, median(vec![1, 0, 9]));
        assert_eq!(5, median(vec![1, 0, 9, 10]));
        assert_eq!(5, median(vec![20, 1, 0, 9]));
        assert_eq!(5, median(vec![20, 1, 0, 10]));
        assert_eq!(6, median(vec![20, 1, 0, 11]));
    }
}
