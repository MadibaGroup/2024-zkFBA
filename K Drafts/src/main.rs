use ark_ff::PrimeField;
use ark_relations::lc;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

#[derive(Clone)]
pub struct CallMarketCircuit<F: PrimeField> {

    pub mcv_column: Vec<Option<F>>,
    pub bid_depth: Vec<Option<F>>,
    pub ask_depth: Vec<Option<F>>,
}

impl<F: PrimeField> ConstraintSynthesizer<F> for CallMarketCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        let n = self.mcv_column.len();

        for i in 0..n {
            let bid = cs.new_witness(|| self.bid_depth[i].ok_or(SynthesisError::AssignmentMissing))?;
            let ask = cs.new_witness(|| self.ask_depth[i].ok_or(SynthesisError::AssignmentMissing))?;
            let mcv = cs.new_input(|| self.mcv_column[i].ok_or(SynthesisError::AssignmentMissing))?;

            if i > 0 { //work on the monotonousity(?)
                // Proof: ask[i] - ask[i-1] >= 0 
                // Proof: bid[i-1] - bid[i] >= 0
                // Full range proofs require bit-decomposition?
            }

            //mcv = min(a, b) in R1CS:
            // (a - mcv) * (b - mcv) == 0  (At least one is equal to MCV?)
            // AND range proofs ensuring (a >= mcv) and (b >= mcv) (?)
            cs.enforce_constraint(
                lc!() + bid - mcv,
                lc!() + ask - mcv,
                lc!(),
            )?;
        }
        Ok(())
    }
}

fn main() {
    use ark_bls12_381::{Bls12_381, Fr};
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::test_rng;

    let mut rng = test_rng();

    let circuit = CallMarketCircuit {
        mcv_column: vec![Some(Fr::from(4000))],
        bid_depth: vec![Some(Fr::from(6000))],
        ask_depth: vec![Some(Fr::from(4000))],
    };

    let (pk, vk) = Groth16::<Bls12_381>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();
    let proof = Groth16::<Bls12_381>::prove(&pk, circuit, &mut rng).unwrap();
    
    let p_input = vec![Fr::from(4000)];
    assert!(Groth16::<Bls12_381>::verify(&vk, &p_input, &proof).unwrap());
    
    println!("Proof verified");
}