use ark_ff::PrimeField;
use ark_relations::lc;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

pub struct PlateauDropOffCircuit<F: PrimeField> {

    pub min_volumes: Vec<Option<F>>, 

    pub v_max: Option<F>,            
    pub v_left: Option<F>,           
    pub v_right: Option<F>,         
    pub c_minus_1: usize,            
    pub d_plus_1: usize,             
}

impl<F: PrimeField> ConstraintSynthesizer<F> for PlateauDropOffCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        let n = self.min_volumes.len();


        let v_max_var = cs.new_input_variable(|| self.v_max.ok_or(SynthesisError::AssignmentMissing))?;
        let v_left_var = cs.new_input_variable(|| self.v_left.ok_or(SynthesisError::AssignmentMissing))?;
        let v_right_var = cs.new_input_variable(|| self.v_right.ok_or(SynthesisError::AssignmentMissing))?;

        let mut min_vars = Vec::new();
        for i in 0..n {
            min_vars.push(cs.new_witness_variable(|| self.min_volumes[i].ok_or(SynthesisError::AssignmentMissing))?);
        }

        
        for i in 0..n {
            let diff_val = self.v_max.and_then(|max| self.min_volumes[i].map(|min| max - min));
         
         
            let mut bit_sum = lc!();
            for j in 0..32 {
                let bit = cs.new_witness_variable(|| {
                    let val = diff_val.ok_or(SynthesisError::AssignmentMissing)?;
                    let u: u64 = val.into_bigint().to_u64_digits()[0];
                    Ok(if (u >> j) & 1 == 1 { F::one() } else { F::zero() })
                })?;

                cs.enforce_constraint(lc!() + bit, lc!() + bit - (F::one(), cs.one()), lc!())?;
                bit_sum = bit_sum + (F::from(1u64 << j), bit);
            }
            cs.enforce_constraint(bit_sum, lc!() + cs.one(), lc!() + v_max_var - min_vars[i])?;
        }

     
        cs.enforce_constraint(lc!() + min_vars[self.c_minus_1] - v_left_var, lc!() + cs.one(), lc!())?;
        
     
        cs.enforce_constraint(lc!() + min_vars[self.d_plus_1] - v_right_var, lc!() + cs.one(), lc!())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_relations::r1cs::ConstraintSystem;
    use ark_test_curves::bls12_381::Fr;

    #[test]
    fn test_plateau_drop_off() {
        let cs = ConstraintSystem::<Fr>::new_ref();

       
        let circuit = PlateauDropOffCircuit {
            min_volumes: vec![
                Some(Fr::from(1u64)),  
                Some(Fr::from(7u64)),  
                Some(Fr::from(12u64)), 
                Some(Fr::from(8u64)),  
                Some(Fr::from(2u64)),  
            ],
            v_max: Some(Fr::from(12u64)),
            v_left: Some(Fr::from(7u64)),
            v_right: Some(Fr::from(8u64)),
            c_minus_1: 1, 
            d_plus_1: 3,  
        };

        circuit.generate_constraints(cs.clone()).unwrap();

        
        assert!(cs.is_satisfied().unwrap(), "Constraints should be satisfied with correct plateau data");
        println!("Number of constraints: {}", cs.num_constraints());
    }
}