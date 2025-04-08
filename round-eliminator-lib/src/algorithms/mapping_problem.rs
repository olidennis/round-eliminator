/// For an explanation of what is happening see https://www.overleaf.com/read/mvttjzgkftbh#0b8190 .

pub mod mapping_problem {
    //use crate::cartesian;
    use anyhow::{anyhow, Result};
    use itertools::structs::MultiProduct;
    use itertools::Itertools;
    use rayon::prelude::*;
    use crate::algorithms::event::EventHandler;
    use crate::algorithms::multisets_pairing::{Comb, Pairings};
    use crate::group::{Group, GroupType, Label};
    use crate::line::Line;
    use crate::part::Part;
    use crate::problem::Problem;
    use std::collections::{HashMap, HashSet};
    use std::iter::Peekable;
    use std::ops::Range;
    use streaming_iterator::StreamingIterator;

    /// Because CartesianProductIterator based on references to the original object
    /// One can not simply just  return a CartesianProductIterator// Assuming you have already defined CartesianProductIterator somewhere
    /// Function that returns an iterator over the Cartesian product of ranges [0..a_1-1], [0..a_2-1], ..., [0..a_n-1]
    pub fn cartesian_product_ranges(a: Vec<usize>) -> MultiProduct<Range<usize>> {
        // Step 1: Create a vector of ranges based on the elements of `a`
        let ranges: Vec<std::ops::Range<usize>> = a.iter().map(|&x| 0..x).collect();

        // Step 2: Return the iterator of the Cartesian product
        ranges.into_iter().multi_cartesian_product()
    }

    /// Given two LCl problems (input, output) it creates between the node configurations of
    /// the input and the output problems a surjective function.
    /// The Iterator trait is also implemented for the struct.
    #[derive(Debug, Clone)]
    pub struct ConfigurationsMapping {
        mappings: Peekable<MultiProduct<Range<usize>>>,
    }

    impl ConfigurationsMapping {
        pub fn new(in_numb: usize, out_numb: usize) -> ConfigurationsMapping {
            ConfigurationsMapping {
                mappings: std::iter::repeat(0..out_numb)
                    .take(in_numb)
                    .multi_cartesian_product()
                    .peekable(),
            }
        }
    }

    impl Iterator for ConfigurationsMapping {
        type Item = Vec<usize>;

        fn next(&mut self) -> Option<Self::Item> {
            self.mappings.next()
        }
    }


    /// Given two LCl problems (input, output) it creates between the label sets of
    /// the input and the output problems a function.
    /// The Iterator trait is also implemented for the struct.
    #[derive(Debug, Clone)]
    pub struct LabelMapping<'a> {
        /// The problem on which we are creating the LabelMapping
        mapping_problem: &'a MappingProblem,
        /// Pairings of lables that make sense with the from_id input node config and to_id output_node_config
        pairings: Vec<(Pairings, usize, usize)>,
        /// For every pairings element all of the possible ones that we need
        /// THIS IS UNUSED, IT IS ONLY FOR TESTING PURPOSES
        all_good_pairings: Vec<Vec<Vec<usize>>>,
        /// Every good_pairings variable it stores the HashSet equivalent of it
        /// So Where does every label goes to in the given node config to node config instance.
        /// So every input node config has a Hashmap which stores every possible pairing matching's result
        /// as a Hashset
        /// Ex. A A X is mapped to B D C then hashmapped_good_pairings[A A X] = [ {A: {B,D}, X: {C}}, {A: {D,C}, X:{D}}, etc. ]
        hashmapped_good_pairings: Vec<Vec<HashMap<Label, HashSet<Label>>>>,
    }

    impl<'a> LabelMapping<'a> {
        /// The current mapping of the input_all_node_config ith Line element to the configurations_map[i]th Line in output_all_node_config
        pub fn new(
            mapping_problem: &'a MappingProblem,
            configurations_map: &Vec<usize>,
        ) -> LabelMapping<'a> {
            let input_all_node_config = &mapping_problem.input_all_node_config_active;
            let output_all_node_config = &mapping_problem.output_all_node_config_active;
            let mut pairs: Vec<(Pairings, usize, usize)> = vec![];
            let line_to_counts = |line: &Line, starvalue: usize| -> Vec<usize> {
                line.parts
                    .iter()
                    .map(|part| match part.gtype {
                        //GroupType::ONE => 1,
                        GroupType::Many(x) => x as usize,
                        GroupType::Star => starvalue,
                    })
                    .collect()
            };

            for (from_id, to_id) in configurations_map.iter().enumerate() {
                let c1 = &input_all_node_config[from_id];
                let c2 = &output_all_node_config[*to_id];
                let v1 = line_to_counts(c1, 0);
                let v2 = line_to_counts(c2, 0);
                pairs.push((Pairings::new(v1, v2), from_id, *to_id));
            }

            LabelMapping {
                mapping_problem: &mapping_problem,
                pairings: pairs,
                all_good_pairings: vec![],
                hashmapped_good_pairings: vec![
                    Vec::new();
                    mapping_problem.input_all_node_config_active.len()
                ],
            }
        }

        fn get_matrix_matching_from_pairing(pairing: &Vec<Comb>) -> Vec<Vec<usize>> {
            let mut matrix: Vec<Vec<usize>> = vec![];
            for v in pairing.iter() {
                matrix.push(v.get().unwrap().clone());
            }
            matrix
        }

        pub fn get_hashmap_version_pairing_matching(
            &self,
            pairing: &(&Vec<Comb>, usize, usize),
        ) -> Result<HashMap<Label, HashSet<Label>>> {
            let matrix = LabelMapping::get_matrix_matching_from_pairing(pairing.0);

            let mut curr_matching: HashMap<Label, HashSet<Label>> = HashMap::new();

            let from_id = pairing.1;
            let from_line = &self.mapping_problem.input_all_node_config_active[from_id];
            let to_id = pairing.2;
            let to_line = &self.mapping_problem.output_all_node_config_active[to_id];

            for (ind_r, row) in matrix.iter().enumerate() {
                let input_group = &from_line.parts[ind_r].group;
                for (ind_e, element) in row.iter().enumerate() {
                    let output_group = &to_line.parts[ind_e].group;
                    if *element != 0 {
                        assert!(input_group.len() == 1);
                        assert!(output_group.len() == 1);
                        curr_matching
                            .entry(input_group.first())
                            .or_insert(HashSet::new())
                            .insert(output_group.first());
                    }
                }
            }

            Ok(curr_matching)
        }

        /// This function iterates over the self.pairings variable and saves every resulting matching in hashed versiom to self.hashmapped_good_pairings
        pub fn hashmapped_pairings_filling(&mut self) {
            let mut pairings_clone = self.pairings.clone();

            for pairing in pairings_clone.iter_mut() {
                while let Some(curr_pairing) = pairing.0.next() {
                    let from_id = pairing.1;
                    let hashed_matching = self
                        .get_hashmap_version_pairing_matching(&(curr_pairing, pairing.1, pairing.2))
                        .unwrap();
                    self.hashmapped_good_pairings[from_id].push(hashed_matching);
                }
            }
        }

        /// This function fills up the self.all_good_pairings and self.hasmapped_good_pairings variables with
        /// Every possible pairing for further use
        pub fn all_possible_pairings_test(&mut self) {
            let mut pairings_clone = self.pairings.clone();

            for pairing in pairings_clone.iter_mut() {
                while let Some(curr_pairing) = pairing.0.next() {
                    self.all_good_pairings.push(
                        LabelMapping::get_matrix_matching_from_pairing(curr_pairing)
                    );
                    println!(
                        "Matching matrix for: {}, to: {}, and the matrix is: {:?}",
                        pairing.1,
                        pairing.2,
                        self.all_good_pairings.last().unwrap()
                    );
                    println!(
                        "Hashmapped version of it: {:?}\n\n\n",
                        self.get_hashmap_version_pairing_matching(&(
                            curr_pairing,
                            pairing.1,
                            pairing.2
                        ))
                    );

                    let from_id = pairing.1;
                    let hashed_matching = self
                        .get_hashmap_version_pairing_matching(&(curr_pairing, pairing.1, pairing.2))
                        .unwrap();
                    self.hashmapped_good_pairings[from_id].push(hashed_matching);
                }
            }
            //println!("I am doing something!");
        }

        /// We wish to test if to_test Hashmap's every element is a subset of containing_map HashMap's every element or not
        pub fn is_hashmap_contained(
            &self,
            to_test: &HashMap<Label, HashSet<Label>>,
            containing_map: &HashMap<Label, HashSet<Label>>,
        ) -> bool {
            for (gr, map_gr) in to_test.iter() {
                let to_compare = containing_map.get(gr).unwrap();
                if !map_gr.is_subset(to_compare) {
                    return false;
                }
            }
            return true;
        }

        /// Currently not the most efficient way of removing for a given input node config the worse mappings
        fn hashed_pairings_reducing_for_config(
            &self,
            from_id: usize,
            label_maps: &Vec<HashMap<Label, HashSet<Label>>>,
        ) -> Vec<HashMap<Label, HashSet<Label>>> {
            let mut keep: Vec<bool> = vec![true; self.hashmapped_good_pairings[from_id].len()];
            for (check_id, maping) in self.hashmapped_good_pairings[from_id].iter().enumerate() {
                for (to_compare_id, maping_comp) in
                    self.hashmapped_good_pairings[from_id].iter().enumerate()
                {
                    if check_id != to_compare_id && keep[to_compare_id] {
                        if self.is_hashmap_contained(maping, maping_comp) {
                            keep[to_compare_id] = false;
                        }
                    }
                }
            }

            let mut new_maps = Vec::new();

            for (ind, v) in keep.iter().enumerate() {
                if *v {
                    new_maps.push(label_maps[ind].clone());
                }
            }

            return new_maps;
        }

        /// For every input node configuration we wish to throw out label_maps that contain even just one other label_map
        pub fn hashed_pairings_reducing(&mut self) {
            let mut new_hashmaping =
                vec![Vec::new(); self.mapping_problem.input_all_node_config_active.len()];
            for (from_id, label_maps) in self.hashmapped_good_pairings.iter().enumerate() {
                new_hashmaping[from_id] =
                    self.hashed_pairings_reducing_for_config(from_id, label_maps);
            }
            self.hashmapped_good_pairings = new_hashmaping;
        }

        ///For every input node config give back a cartesian product of possible mappings to the ouput config in the current
        /// Configuations mapping and create a Cartesian product on which we can iterate over
        pub fn cartesian_choices_hashed(&self) -> MultiProduct<Range<usize>> {
            let cartesian: Vec<usize> = self
                .hashmapped_good_pairings
                .iter()
                .map(|v| v.len()) // Map each element to its length
                .collect(); // Collect the results into a Vec<usize>

            cartesian_product_ranges(cartesian)
        }

        /// Given a vector with indices pointing to hashmapped_good_pairings different elements
        /// so chosen_label_maps[i] is in 0..hashmapped_good_pairings[i]
        /// returns for every a Hashmap<label, Hashset<label>> so for
        /// every input label the possible output labels in this label mapping
        pub fn possible_labels(
            &self,
            chosen_label_maps: &Vec<usize>,
        ) -> HashMap<Label, HashSet<Label>> {
            let mut result = HashMap::new();

            for (indi_v, v) in chosen_label_maps.iter().enumerate() {
                let map = &self.hashmapped_good_pairings[indi_v][*v];
                for (key, value_set) in map {
                    // Entry API allows updating the value if the key already exists
                    result
                        .entry((*key).clone())
                        .or_insert_with(HashSet::new)
                        .extend(value_set.into_iter().cloned());
                }
            }
            result
        }

        /// Given an edge from the input problem and the possible labelings that are achieved by the current $$f$$ config mapping and the label mappings $$g_l$$
        /// returns all of the possible edges as a Line
        pub fn possible_edges(
            &self,
            edge_config: &Line,
            possible_labelings: &HashMap<Label, HashSet<Label>>,
        ) -> Line {

            // Process each part of the edge, considering possible_labelings
            let parts_for_line = edge_config.parts
                .iter()
                .map(|part| {
                    let mut union = HashSet::new();
                    for label in part.group.iter() {
                        let label_set = &possible_labelings[&label];
                        union.extend(label_set.iter().cloned());
                    }
                    let group = Group::from_set(&union);

                    Part {
                        gtype: part.gtype,
                        group,
                    }
                })
                .collect(); // Join all parts with spaces to represent the full edge

            Line {
                parts: parts_for_line,
            }
        }

        pub fn all_good_pairings(&self) -> &Vec<Vec<Vec<usize>>> {
            return &self.all_good_pairings;
        }

        pub fn hashmapped_good_pairings(&self) -> &Vec<Vec<HashMap<Label, HashSet<Label>>>> {
            return &self.hashmapped_good_pairings;
        }

        pub fn pairings(&self) -> &Vec<(Pairings, usize, usize)> {
            return &self.pairings;
        }
    }


    /// Given two LCl problems (input, output) we whis to find a f:N_in -> N_out
    /// and \forall l \in N_in g_l: l -> f(l) set parition function such that
    /// only allowed edge configurations remain
    #[derive(Debug, Clone)]
    pub struct MappingProblem {
        pub input_problem: Problem,
        pub output_problem: Problem,
        //Every possible node configuration for the input problem
        pub input_all_node_config_active: Vec<Line>,
        //Every possible node configuration for the output problem
        pub output_all_node_config_active: Vec<Line>,
        configurations_map: ConfigurationsMapping,
    }

    impl MappingProblem {
        pub fn new(in_p: Problem, out_p: Problem) -> MappingProblem {
            let input_all_node_config = in_p.active.all_choices(true);
            let output_all_node_config = out_p.active.all_choices(true);
            let in_p_size = input_all_node_config.len();
            let out_p_size = output_all_node_config.len();
            MappingProblem {
                input_problem: in_p,
                output_problem: out_p,
                input_all_node_config_active: input_all_node_config,
                output_all_node_config_active: output_all_node_config,
                configurations_map: ConfigurationsMapping::new(in_p_size, out_p_size),
            }
        }

        /// Just writes out the .all_choices() function results of the input and output problems
        pub fn long_describ_problems(&self) {
            println!(
                "Input active: {:?}",
                self.input_problem.active.all_choices(true)
            );
            let mapping = self
                .input_problem
                .mapping_label_text
                .iter()
                .cloned()
                .collect();
            for v in self.input_problem.active.all_choices(true) {
                println!("{}", v.to_string(&mapping));
            }

            println!(
                "Input passive: {:?}",
                self.input_problem.active.all_choices(true)
            );
            let mapping = self
                .input_problem
                .mapping_label_text
                .iter()
                .cloned()
                .collect();
            for v in self.input_problem.passive.all_choices(true) {
                println!("{}", v.to_string(&mapping));
            }
            //println!("{:?}", self.input_problem.passive.all_choices(true));

            //println!("{:?}", self.output_problem.active.all_choices(true));
            println!(
                "Output active: {:?}",
                self.output_problem.active.all_choices(true)
            );
            let mapping = self
                .output_problem
                .mapping_label_text
                .iter()
                .cloned()
                .collect();
            for v in self.output_problem.active.all_choices(true) {
                println!("{}", v.to_string(&mapping));
            }

            println!(
                "Output passive: {:?}",
                self.output_problem.active.all_choices(true)
            );
            let mapping = self
                .output_problem
                .mapping_label_text
                .iter()
                .cloned()
                .collect();
            for v in self.output_problem.passive.all_choices(true) {
                println!("{}", v.to_string(&mapping));
            }
        }

        pub fn print_input_line_config(&self, line: &Line) -> String {
            let mapping = self
                .input_problem
                .mapping_label_text
                .iter()
                .cloned()
                .collect();

            line.to_string(&mapping)
        }

        pub fn print_output_line_config(&self, line: &Line) -> String {
            let mapping = self
                .output_problem
                .mapping_label_text
                .iter()
                .cloned()
                .collect();

            line.to_string(&mapping)
        }

        /// Given the ConfigurationsMapping and LabelMapping it creates for
        /// every input problem label the possible output labels
        fn summarized_labelling() -> HashMap<Group, HashSet<Group>> {
            let mut summarized_labels = HashMap::new();

            summarized_labels
        }

        /// Given a map of the node configs it creates a LabelMapping variable
        pub fn labelmapping_from_the_config(
            &self,
            config_map: &Vec<usize>,
        ) -> LabelMapping {
            LabelMapping::new(
                &self,
                &config_map,
            )
        }

        pub fn next_config(&mut self) -> Option<Vec<usize>> {
            self.configurations_map.next()
        }

        pub fn maximize_out_problem(&mut self) {
            self.output_problem
                .passive
                .maximize(&mut EventHandler::null());
        }

        pub fn search_for_mapping(&mut self) -> Option<Vec<(Label, HashSet<Label>)>> {
            #[cfg(target_arch = "wasm32")]
            { self.search_for_mapping_sequential() }
            #[cfg(not(target_arch = "wasm32"))]
            { self.search_for_mapping_parallel() }
        }

        /// Tries to find a correct mapping configuration, that solves the problem.
        #[cfg(target_arch = "wasm32")]
        pub fn search_for_mapping_sequential(&mut self) -> Option<Vec<(Label, HashSet<Label>)>> {
            while let Some(curr_config) = self.next_config() {
                if cfg!(debug_assertions) {
                    println!("Current config mapping: {:?}", curr_config);
                }

                let mut label_map = self.labelmapping_from_the_config(&curr_config);
                label_map.hashmapped_pairings_filling();

                if cfg!(debug_assertions) {
                    println!(
                        "Every hashmapping for every node configuration: {:?}",
                        label_map.hashmapped_good_pairings()
                    );
                }

                let mut cartesian_labels_poss = label_map.cartesian_choices_hashed();

                while let Some(curr) = cartesian_labels_poss.next() {
                    // Get the possible labels for the current configuration
                    let possible_labels = label_map.possible_labels(&curr);
                    if cfg!(debug_assertions) {
                        println!("\tPossible labels: {:?}", possible_labels);
                    }

                    let mut possible = true;
                    for edge_config in &self.input_problem.passive.lines {
                        let line_edge = label_map.possible_edges(edge_config, &possible_labels);
                        if !self.output_problem.passive.includes(&line_edge) {
                            possible = false;
                            //println!("\t\t\tFailed to find a possible mapping HERE.");
                            break;
                        }
                    }

                    if possible {
                        #[cfg(debug_assertions)]
                        {
                            println!("\t\tFound a possible mapping");
                            println!("\t\tConfig Mapping: {:?}", curr_config);
                            println!("\t\tEdges: {:?}", possible_labels);
                            println!("\t\tMapping: {:?}", curr);
                        }
                        return Some(possible_labels.into_iter().collect());
                    } else {
                        #[cfg(debug_assertions)]
                        {
                            println!("\t\tFailed to find a possible mapping");
                        }
                    }
                }
            }
            return None;
        }

        /// Tries to find a correct mapping configuration, that solves the problem in parallel.
        #[cfg(not(target_arch = "wasm32"))]
        pub fn search_for_mapping_parallel(&mut self) -> Option<Vec<(Label, HashSet<Label>)>> {
            // Collect all configurations from `next_config` into a vector
            let configs: Vec<_> = std::iter::from_fn(|| self.next_config()).collect();

            // Use Rayon to process configurations in parallel
            let found = configs.par_iter().find_map_any(|curr_config| {
                if cfg!(debug_assertions) {
                    println!("Current config mapping: {:?}", curr_config);
                }
        
                let mut label_map = self.labelmapping_from_the_config(curr_config);
                label_map.hashmapped_pairings_filling();
                label_map.hashed_pairings_reducing();
        
                if cfg!(debug_assertions) {
                    println!("Every hashmapping for every node configuration: {:?}", label_map.hashmapped_good_pairings());
                }
        
                let mut cartesian_labels_poss = label_map.cartesian_choices_hashed();
        
                while let Some(curr) = cartesian_labels_poss.next() {
                    // Get the possible labels for the current configuration
                    let possible_labels = label_map.possible_labels(&curr);
        
                    if cfg!(debug_assertions) {
                        println!("\tPossible labels: {:?}", possible_labels);
                    }
        
                    let mut possible = true;
                    for edge_config in &self.input_problem.passive.lines {
                        let line_edge = label_map.possible_edges(edge_config, &possible_labels);
                        if !self.output_problem.passive.includes(&line_edge) {
                            possible = false;
                            //println!("\t\t\tFailed to find a possible mapping HERE.");
                            break;
                        }
                    }
        
                    if possible {
                        #[cfg(debug_assertions)]
                        {
                            println!("\t\tFound a possible mapping");
                            println!("\t\tConfig Mapping: {:?}", curr_config);
                            println!("\t\tEdges: {:?}", possible_labels);
                            println!("\t\tMapping: {:?}", curr);
                        }
                        return Some(possible_labels.into_iter().collect());
                    } else {
                        #[cfg(debug_assertions)]
                        {
                            println!("\t\tFailed to find a possible mapping");
                        }
                    }
                }
                None
            });

            found
        }

        /// A testing function to see what does Pairings do.
        pub fn label_mapping_for_config(&mut self) {
            while let Some(curr_config) = self.configurations_map.next() {
                let mut label_map = self.labelmapping_from_the_config(&curr_config);

                if cfg!(debug_assertions) {
                    println!("Current configurations mapping: {:?}", curr_config);
                }
                label_map.all_possible_pairings_test();
                if cfg!(debug_assertions) {
                    println!(
                        "All possible pairings in matrix form for this configuration mapping: {:?}",
                        label_map.all_good_pairings()
                    );

                    println!(
                        "All possible pairings in hashmapped form: {:?}",
                        label_map.hashmapped_good_pairings()
                    );
                }
                //break;
            }

        }
    }
}

#[cfg(test)]
mod tests {

    use itertools::Itertools;
    use permutator::CartesianProductIterator;
    use crate::algorithms::event::EventHandler;
    use crate::group::{Group, GroupType, Label};
    use crate::line::Line;
    use crate::part::Part;
    use std::collections::{HashMap, HashSet};
    use streaming_iterator::StreamingIterator;

    use super::{
        mapping_problem::{ConfigurationsMapping, MappingProblem},
        *,
    };

    use std::time::{Duration, Instant};

    use crate::problem::{self, Problem};

    #[test]
    fn cartesian_prod() {
        let mut multi_prod = (0..3)
            .map(|i| (i * 2)..(i * 2 + 2))
            .multi_cartesian_product();
        assert_eq!(multi_prod.next(), Some(vec![0, 2, 4]));
        assert_eq!(multi_prod.next(), Some(vec![0, 2, 5]));
        assert_eq!(multi_prod.next(), Some(vec![0, 3, 4]));
        assert_eq!(multi_prod.next(), Some(vec![0, 3, 5]));
        assert_eq!(multi_prod.next(), Some(vec![1, 2, 4]));
        assert_eq!(multi_prod.next(), Some(vec![1, 2, 5]));
        assert_eq!(multi_prod.next(), Some(vec![1, 3, 4]));
        assert_eq!(multi_prod.next(), Some(vec![1, 3, 5]));
        assert_eq!(multi_prod.next(), None);

        for v in (0..3).map(|i| (i * 2)..(i * 2 + 2)) {
            println!("Val: {:?}", v);
        }
    }

    #[test]
    fn cartesian_changing_size() {
        let test = ConfigurationsMapping::new(3, 2);
        for v in test {
            println!("Current mapping: {:?}", v);
        }
    }

    #[test]
    fn varying_cartesian_product() {
        let n: usize = 5;
        let bm: Vec<&[usize]> = vec![&[0, 1, 2]; n];
        let test: CartesianProductIterator<usize> = CartesianProductIterator::new(&bm);
        for cr in CartesianProductIterator::new(&bm) {
            println!("{:?}", cr);
        }
    }

    #[test]
    fn problems_long_describ() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A AX Y \nB B BY A\n\nAB CD").unwrap(),
            Problem::from_string("A B B D\nC D D A\n\nAB CD").unwrap(),
        );
        test.long_describ_problems();

        test.label_mapping_for_config();
    }

    #[test]
    fn all_pairings_listing() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A AX Y \nB B BY A\n\nAB CD").unwrap(),
            Problem::from_string("A B B D\nC D D A\n\nAB CD").unwrap(),
        );
    }

    #[test]
    fn hasmapping_from_pairing() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A AX Y \nB B BY A\n\nAB CD").unwrap(),
            Problem::from_string("A B B D\nC D D A\n\nAB CD").unwrap(),
        );
        test.long_describ_problems();

        let curr_config = test.next_config().unwrap();
        println!("Current config mapping: {:?}", curr_config);
        let label_map = test.labelmapping_from_the_config(&curr_config);
        let mut current_pairing = label_map.pairings();
        let mut one_pairing = current_pairing[0].clone();
        println!(
            "Mapping from input line: {:?} to output line: {:?} with line describ {:?} to {:?}",
            test.print_input_line_config(&test.input_all_node_config_active[one_pairing.1]),
            test.print_output_line_config(&test.output_all_node_config_active[one_pairing.2]),
            test.input_all_node_config_active[one_pairing.1],
            test.output_all_node_config_active[one_pairing.2]
        );

        let true_asnwers: Vec<HashMap<Label, HashSet<Label>>> = vec![
            HashMap::from([
                (
                    0,
                    HashSet::from([2,1]),
                ),
                (2, HashSet::from([0])),
            ]),
            HashMap::from([
                (
                    0,
                    HashSet::from([0,1]),
                ),
                (2, HashSet::from([2])),
            ]),
            HashMap::from([
                (
                    0,
                    HashSet::from([0,1,2]),
                ),
                (2, HashSet::from([1])),
            ]),
        ];
        let mut func_res = Vec::new();
        while let Some(v) = one_pairing.0.next() {
            func_res.push(
                label_map
                    .get_hashmap_version_pairing_matching(&(v, one_pairing.1, one_pairing.2))
                    .unwrap(),
            );
            println!("Current hashed mapping: {:?}", func_res.last().unwrap());
        }

        assert_eq!(true_asnwers, func_res);
    }

    #[test]
    fn hashmap_containment() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A AX Y \nB B BY A\n\nAB CD").unwrap(),
            Problem::from_string("A B B D\nC D D A\n\nAB CD").unwrap(),
        );
        test.long_describ_problems();
        let curr_config = test.next_config().unwrap();
        println!("Current config mapping: {:?}", curr_config);
        let mut label_map = test.labelmapping_from_the_config(&curr_config);

        label_map.hashmapped_pairings_filling();
        let two_hasmaps = vec![
            HashMap::from([
                (
                    0,
                    HashSet::from([2,0]),
                ),
                (2, HashSet::from([0])),
            ]),
            HashMap::from([
                (
                    0,
                    HashSet::from([2,1,0]),
                ),
                (2, HashSet::from([0])),
            ]),
        ];
        println!(
            "Are these two hashmaps contained in each other? Small: {:?}, Big: {:?}",
            two_hasmaps[0], two_hasmaps[1]
        );
        assert!(label_map.is_hashmap_contained(&two_hasmaps[0], &two_hasmaps[1]));
    }

    #[test]
    fn hasmaps_reduction() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A AX Y Y\n\nAB CD").unwrap(),
            Problem::from_string("A B B D A\n\nAB CD").unwrap(),
        );
        test.long_describ_problems();
        let curr_config = test.next_config().unwrap();
        println!("Current config mapping: {:?}", curr_config);
        let mut label_map = test.labelmapping_from_the_config(&curr_config);

        let removed = HashMap::from([
            (
                0,
                HashSet::from([2,1,0]),
            ),
            (
                2,
                HashSet::from([0,1]),
            ),
        ]);

        label_map.hashmapped_pairings_filling();
        println!(
            "Every hashmapping for every node configuration: {:?}",
            label_map.hashmapped_good_pairings()[0]
        );
        assert!(label_map.hashmapped_good_pairings()[0].contains(&removed));
        label_map.hashed_pairings_reducing();
        println!(
            "Every hashmapping for every node configuration: {:?}",
            label_map.hashmapped_good_pairings()[0]
        );
        assert!(!label_map.hashmapped_good_pairings()[0].contains(&removed));
    }

    #[test]
    fn iterate_cartesians() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A AX Y Y\n\nAB CD").unwrap(),
            Problem::from_string("A B B D A\n\nAB CD").unwrap(),
        );
        test.long_describ_problems();
        let curr_config = test.next_config().unwrap();
        println!("Current config mapping: {:?}", curr_config);
        let mut label_map = test.labelmapping_from_the_config(&curr_config);

        let removed = HashMap::from([
            (
                0,
                HashSet::from([2,1,0]),
            ),
            (
                2,
                HashSet::from([0,1]),
            ),
        ]);

        label_map.hashmapped_pairings_filling();
        println!(
            "Every hashmapping for every node configuration: {:?}",
            label_map.hashmapped_good_pairings()[0]
        );
        assert!(label_map.hashmapped_good_pairings()[0].contains(&removed));
        label_map.hashed_pairings_reducing();
        println!(
            "Every hashmapping for every node configuration: {:?}",
            label_map.hashmapped_good_pairings()[0]
        );
        assert!(!label_map.hashmapped_good_pairings()[0].contains(&removed));

        println!("\n\n");
        for (indi, v) in label_map.hashmapped_good_pairings().iter().enumerate() {
            println!(
                "The indi: {:?}, have the following hasmaps: {:?}, have the size of {:?}",
                indi,
                v,
                v.len()
            );
        }

        let mut all_maps_combinations = label_map.cartesian_choices_hashed();
        for i in 0..4 {
            for j in 0..10 {
                assert_eq!(all_maps_combinations.next(), Some(vec![i, j]));
            }
        }
    }

    #[test]
    fn iterate_cartesians_1def_2coloring() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A A\nA A X\nB B B\nB B Y\n\nA B\nA Y\nX B\nX Y\nX X\nX Y\nY Y")
                .unwrap(),
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap(),
        );
        test.long_describ_problems();
        let curr_config = test.next_config().unwrap();
        println!("Current config mapping: {:?}", curr_config);
        let mut label_map = test.labelmapping_from_the_config(&curr_config);

        label_map.hashmapped_pairings_filling();
        println!(
            "Every hashmapping for every node configuration: {:?}",
            label_map.hashmapped_good_pairings()
        );
        label_map.hashed_pairings_reducing();
        println!(
            "Every hashmapping for every node configuration after reduction: {:?}",
            label_map.hashmapped_good_pairings()
        );

        println!("\n\n");
        for (indi, v) in label_map.hashmapped_good_pairings().iter().enumerate() {
            println!(
                "The indi: {:?}, have the following hasmaps: {:?}, have the size of {:?}",
                indi,
                v,
                v.len()
            );
        }

        let mut cartesian_labels_poss = label_map.cartesian_choices_hashed();

        let curr = cartesian_labels_poss.next().unwrap();
        println!(
            "Current summarized label mapping: {:?}, with cartesian choosing of: {:?}",
            label_map.possible_labels(&curr),
            curr
        );

        let map_corr: HashMap<Label, HashSet<Label>> = HashMap::from([
            (1, HashSet::from([1])),
            (3, HashSet::from([1])),
            (
                0,
                HashSet::from([0,1]),
            ),
            (
                2,
                HashSet::from([1,0]),
            ),
        ]);

        assert_eq!(map_corr, label_map.possible_labels(&curr));
    }

    #[test]
    fn iterate_cartesians_MIS() {
        let mut test = MappingProblem::new(
            Problem::from_string("M M M\nP O O\n\nM P\nO OM").unwrap(),
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap(),
        );
        test.long_describ_problems();
        let curr_config = test.next_config().unwrap();
        println!("Current config mapping: {:?}", curr_config);
        let mut label_map = test.labelmapping_from_the_config(&curr_config);

        label_map.hashmapped_pairings_filling();
        println!(
            "Every hashmapping for every node configuration: {:?}",
            label_map.hashmapped_good_pairings()
        );
        label_map.hashed_pairings_reducing();
        println!(
            "Every hashmapping for every node configuration after reduction: {:?}",
            label_map.hashmapped_good_pairings()
        );

        println!("\n\n");
        for (indi, v) in label_map.hashmapped_good_pairings().iter().enumerate() {
            println!(
                "The indi config: {:?}, have the following hasmaps: {:?}, have the size of {:?}",
                indi,
                v,
                v.len()
            );
        }

        let mut cartesian_labels_poss = label_map.cartesian_choices_hashed();

        let mut curr = cartesian_labels_poss.next().unwrap();
        curr = cartesian_labels_poss.next().unwrap();
        println!(
            "Current summarized label mapping: {:?}, with cartesian choosing of: {:?}",
            label_map.possible_labels(&curr),
            curr
        );

        let map_corr: HashMap<Label, HashSet<Label>> = HashMap::from([
            (1, HashSet::from([0])),
            (
                2,
                HashSet::from([1,0]),
            ),
            (
                0,
                HashSet::from([1,0]),
            ),
        ]);

        assert_eq!(map_corr, label_map.possible_labels(&curr));
    }

    #[test]
    fn iterate_cartesians_MIS_edges() {
        let out_p =
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap();
        //out_p.maximi
        let mut test = MappingProblem::new(
            Problem::from_string("M M M\nP O O\n\nM P\nO OM").unwrap(),
            out_p,
        );
        test.long_describ_problems();
        let curr_config = test.next_config().unwrap();
        println!("Current config mapping: {:?}", curr_config);
        let mut label_map = test.labelmapping_from_the_config(&curr_config);

        label_map.hashmapped_pairings_filling();
        println!(
            "Every hashmapping for every node configuration: {:?}",
            label_map.hashmapped_good_pairings()
        );
        label_map.hashed_pairings_reducing();
        println!(
            "Every hashmapping for every node configuration after reduction: {:?}",
            label_map.hashmapped_good_pairings()
        );

        println!("\n\n");
        for (indi, v) in label_map.hashmapped_good_pairings().iter().enumerate() {
            println!(
                "The indi config: {:?}, have the following hasmaps: {:?}, have the size of {:?}",
                indi,
                v,
                v.len()
            );
        }

        let mut cartesian_labels_poss = label_map.cartesian_choices_hashed();

        while let Some(curr) = cartesian_labels_poss.next() {
            // Get the possible labels for the current configuration
            let possible_labels = label_map.possible_labels(&curr);

            for edge_config in &test.input_problem.passive.lines {
                let edges = label_map.possible_edges(edge_config, &possible_labels);
                // Print the edge index and corresponding edges
                println!(
                    "Edge config: {:?}, Current label mapping: {:?}, Possible edges: {:?}",
                    edge_config, curr, edges
                );
            }
        }
    }

    #[test]
    fn iterate_cartesians_1def_2coloring_edges() {
        let mut out_p =
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap();
        out_p.passive.maximize(&mut EventHandler::null());
        let mut test = MappingProblem::new(
            Problem::from_string("A A A\nA A X\nB B B\nB B Y\n\nA B\nA Y\nX B\nX Y\nX X\nX Y\nY Y")
                .unwrap(),
            out_p.clone(),
        );
        test.long_describ_problems();
        let curr_config = test.next_config().unwrap();
        println!("Current config mapping: {:?}", curr_config);
        let mut label_map = test.labelmapping_from_the_config(&curr_config);

        label_map.hashmapped_pairings_filling();
        println!(
            "Every hashmapping for every node configuration: {:?}",
            label_map.hashmapped_good_pairings()
        );
        label_map.hashed_pairings_reducing();
        println!(
            "Every hashmapping for every node configuration after reduction: {:?}",
            label_map.hashmapped_good_pairings()
        );

        println!("\n\n");
        for (indi, v) in label_map.hashmapped_good_pairings().iter().enumerate() {
            println!(
                "The indi: {:?}, have the following hasmaps: {:?}, have the size of {:?}",
                indi,
                v,
                v.len()
            );
        }

        let mut cartesian_labels_poss = label_map.cartesian_choices_hashed();

        while let Some(curr) = cartesian_labels_poss.next() {
            // Get the possible labels for the current configuration
            let possible_labels = label_map.possible_labels(&curr);

            println!("Possible labels for gruops: {:?}", possible_labels);


            for edge_config in &test.input_problem.passive.lines {
                let edges = label_map.possible_edges(edge_config, &possible_labels);
                // Print the edge index and corresponding edges
                println!(
                    "Edge config: {:?}, Current label mapping: {:?}, Possible edges: {:?}",
                    edge_config, curr, edges
                );
            }

        }
    }

    #[test]
    fn iterate_cartesians_MIS_solution_search() {
        let mut test = MappingProblem::new(
            Problem::from_string("M M M\nP O O\n\nM P\nO OM").unwrap(),
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap(),
        );

        test.maximize_out_problem();

        test.long_describ_problems();

        assert!(test.search_for_mapping().is_some());
    }

    #[test]
    fn no_solution() {
        let mut test = MappingProblem::new(
            Problem::from_string("M M M\nP O O\n\nM P\nO OM").unwrap(),
            Problem::from_string("T H HT\nT T H\nT T T\n\nH T").unwrap(),
        );

        test.maximize_out_problem();

        test.long_describ_problems();

        assert_eq!(test.search_for_mapping().is_some(), false);
    }

    #[test]
    fn one_way_possible_labelling() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A A\nA A X\nB B B\nB B Y\n\nA B\nA Y\nX B\nX Y\nX X\nX Y\nY Y")
                .unwrap(),
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap(),
        );

        test.maximize_out_problem();

        test.long_describ_problems();

        assert!(test.search_for_mapping().is_some());
    }

    #[test]
    fn one_way_possible_labelling_compact_form() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A AX\nB B BY\n\nA YB\nX YBX\nY Y").unwrap(),
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap(),
        );

        test.maximize_out_problem();

        test.long_describ_problems();

        assert!(test.search_for_mapping().is_some());
    }

    #[test]
    fn one_way_possible_harder_labelling() {
        let mut test = MappingProblem::new(
            Problem::from_string(
                "A A A A A\nA A A A X\nB B B B B\nB B B B Y\n\nA B\nA Y\nX B\nX Y\nX X\nX Y\nY Y",
            )
            .unwrap(),
            Problem::from_string("A A A A X\nB B B B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y")
                .unwrap(),
        );

        test.maximize_out_problem();

        test.long_describ_problems();

        assert_eq!(true, test.search_for_mapping().is_some());
        assert_eq!(true, test.search_for_mapping_parallel().is_some());

        let mut test_backward = MappingProblem::new(
            Problem::from_string("A A A A X\nB B B B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y")
                .unwrap(),
            Problem::from_string(
                "A A A A A\nA A A A X\nB B B B B\nB B B B Y\n\nA B\nA Y\nX B\nX Y\nX X\nX Y\nY Y",
            )
            .unwrap(),
        );

        test_backward.maximize_out_problem();

        test_backward.long_describ_problems();

        assert_eq!(false, test_backward.search_for_mapping().is_some());
        assert_eq!(false, test_backward.search_for_mapping_parallel().is_some());
    }

    #[test]
    fn one_way_possible_labelling_compact_form_large() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A A A A AX\nB B B B B BY\n\nA YB\nX YBX\nY Y").unwrap(),
            Problem::from_string(
                "A A A A A X\nB B B B B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y",
            )
            .unwrap(),
        );

        test.maximize_out_problem();

        test.long_describ_problems();

        assert!(test.search_for_mapping().is_some());
    }

    #[test]
    fn parallel_vs_sequential() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A A A A AX\nB B B B B BY\n\nA YB\nX YBX\nY Y").unwrap(),
            Problem::from_string(
                "A A A A A X\nB B B B B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y",
            )
            .unwrap(),
        );

        test.maximize_out_problem();

        test.long_describ_problems();

        let start = Instant::now();

        assert!(test.search_for_mapping().is_some());
        let duration = start.elapsed();

        println!("Time elapsed in expensive_function() is: {:?}", duration);

        let start = Instant::now();

        assert!(test.search_for_mapping_parallel().is_some());
        let duration = start.elapsed();

        println!("Time elapsed in expensive_function() is: {:?}", duration);
    }

    #[test]
    fn includes_testing() {
        let mut test = MappingProblem::new(
            Problem::from_string("A A A\nA A X\nB B B\nB B Y\n\nA B\nA Y\nX B\nX Y\nX X\nX Y\nY Y")
                .unwrap(),
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap(),
        );

        test.long_describ_problems();
        let prob =
            Problem::from_string("A A X\nB B Y\n\nA B\nA A\nA Y\nB X\nB B\nX B\nY A\nX Y").unwrap();

        let l1 = Line {
            parts: vec![
                Part {
                    gtype: GroupType::ONE,
                    group: Group::from(vec![2, 3]),
                },
                Part {
                    gtype: GroupType::ONE,
                    group: Group::from(vec![0, 1]),
                },
            ],
        };

        assert_eq!(true, prob.passive.includes(&l1));

        let l2 = Line {
            parts: vec![
                Part {
                    gtype: GroupType::ONE,
                    group: Group::from(vec![2, 3]),
                },
                Part {
                    gtype: GroupType::ONE,
                    group: Group::from(vec![1, 0]),
                },
            ],
        };

        assert_eq!(false, prob.passive.includes(&l2));

        //let l2 = Line::parse("BY A", &mut HashMap::new()).unwrap();
        //let l3 = Line::parse("B XA", &mut HashMap::new()).unwrap();
        //let l4 = Line::parse("B A", &mut HashMap::new()).unwrap();
        //println!("Testing the edge: {:?}", l1);
        //assert!(prob.passive.includes(&l1));
    }
}
