// SPDX-License-Identifier: MPL-2.0
//! Ranks for `ww3_shel`, as WW3's card-deck `DIST` distributes them (`W3_DIST W3_MPI`): sea points go
//! round-robin (`INIT_GET_ISEA`, `w3parall.F90:1256`), spectral components go by `IAPPRO` (`w3initmd.F90:853`),
//! and `W3GATH`/`W3SCAT` move a component's field between the owners of the points and the owner of the
//! component. Every rank keeps the whole state and computes its own part, so a rank layout cannot reach an
//! arithmetic operation; the build without the `mpi` feature is a one-rank `Par` and calls no MPI.

use ww3_kernels::real::Real;

/// This process's place among the ranks. `size() == 1` is the serial run.
pub struct Par {
    rank: usize,
    size: usize,
    #[cfg(feature = "mpi")]
    world: Option<mpi::topology::SimpleCommunicator>,
    // Last, so that MPI_Finalize runs after the communicator is freed.
    #[cfg(feature = "mpi")]
    _universe: Option<mpi::environment::Universe>,
}

impl Par {
    pub fn serial() -> Self {
        Self {
            rank: 0,
            size: 1,
            #[cfg(feature = "mpi")]
            world: None,
            #[cfg(feature = "mpi")]
            _universe: None,
        }
    }

    /// Joins the ranks `mpirun` started. A process that was not started by a launcher, and a build
    /// without the `mpi` feature, get `serial()` and never call `MPI_Init`.
    pub fn init() -> Self {
        #[cfg(feature = "mpi")]
        {
            use mpi::traits::Communicator;
            let launched = ["OMPI_COMM_WORLD_SIZE", "PMI_SIZE", "PMIX_RANK"].iter().any(|v| std::env::var_os(v).is_some());
            if launched {
                // The ranks call MPI from the main thread only, between their own thread scopes.
                let (universe, provided) = mpi::initialize_with_threading(mpi::Threading::Funneled).expect("MPI_Init failed");
                assert!(provided >= mpi::Threading::Funneled, "MPI does not give MPI_THREAD_FUNNELED");
                let world = universe.world();
                return Self {
                    rank: world.rank() as usize,
                    size: world.size() as usize,
                    world: Some(world),
                    _universe: Some(universe),
                };
            }
        }
        Self::serial()
    }

    pub fn rank(&self) -> usize {
        self.rank
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// `ISEA = IAPROC + (JSEA-1)*NAPROC` with `IAPROC = rank + 1`.
    pub fn mine(&self, isea: usize) -> bool {
        (isea - 1) % self.size == self.rank
    }

    /// `MPI_Alltoallv` of `Real`s: block `q` of `send` (`scnt[q]` values) goes to rank `q`, and block
    /// `p` of `recv` (`rcnt[p]`) comes from rank `p`. Bits are moved, never combined.
    #[allow(unused_variables)]
    pub fn alltoallv(&self, send: &[Real], scnt: &[usize], recv: &mut [Real], rcnt: &[usize]) {
        #[cfg(feature = "mpi")]
        {
            use mpi::collective::CommunicatorCollectives;
            use mpi::datatype::{Partition, PartitionMut};
            let offsets = |c: &[usize]| -> (Vec<i32>, Vec<i32>) {
                let mut at = 0;
                let (mut n, mut d) = (Vec::new(), Vec::new());
                for &x in c {
                    n.push(x as i32);
                    d.push(at as i32);
                    at += x;
                }
                (n, d)
            };
            let (sn, sd) = offsets(scnt);
            let (rn, rd) = offsets(rcnt);
            assert_eq!(scnt.iter().sum::<usize>(), send.len());
            assert_eq!(rcnt.iter().sum::<usize>(), recv.len());
            let sp = Partition::new(send, sn, sd);
            let mut rp = PartitionMut::new(recv, rn, rd);
            self.world.as_ref().expect("no communicator").all_to_all_varcount_into(&sp, &mut rp);
        }
        #[cfg(not(feature = "mpi"))]
        unreachable!("one rank never communicates");
    }
}

/// Who owns what. `pts[p]` are the 1-based sea points of rank `p`, `specs[p]` its 0-based components.
pub struct Layout {
    pub pts: Vec<Vec<usize>>,
    pub specs: Vec<Vec<usize>>,
}

impl Layout {
    #[allow(clippy::too_many_arguments)]
    pub fn new(naproc: usize, nsea: usize, nspec: usize, nk: usize, nth: usize, dtmax: Real, dtcfl: Real, sig: &[Real]) -> Self {
        let pts = (0..naproc).map(|p| (1..=nsea).filter(|i| (i - 1) % naproc == p).collect()).collect();
        let owner = iappro(naproc, nspec, nk, nth, dtmax, dtcfl, sig);
        let specs = (0..naproc).map(|p| (0..nspec).filter(|&i| owner[i] == p).collect()).collect();
        Self { pts, specs }
    }
}

/// `IAPPRO` of `W3INIT` (`w3initmd.F90:843-915`): components dealt out in a boustrophedon over the ranks
/// until each rank holds about `NTTARG` propagation sub-steps. The result is a rank number from 0.
#[allow(clippy::needless_range_loop)]
pub fn iappro(naproc: usize, nspec: usize, nk: usize, nth: usize, dtmax: Real, dtcfl: Real, sig: &[Real]) -> Vec<usize> {
    let ntloc = |ik: usize| 1 + (dtmax / (dtcfl * sig[ik] / sig[1]) - 0.001) as i64;
    let nttot: i64 = (1..=nk).map(|ik| ntloc(ik) * nth as i64).sum();
    let na = naproc as i64;
    let mut nttarg = 1 + (nttot - 1) / na + (dtmax / (dtcfl * sig[nk] / sig[1]) - 0.001) as i64;
    let nttmax = nttarg + 5;
    let mut ia = vec![1i64; nspec + 1];
    let mut nt = vec![0i64; naproc + 1];
    loop {
        for ip in 1..=naproc {
            let (mut istep, mut isp) = (ip, 0);
            nt[ip] = 0;
            for j in 1..=1 + nspec / naproc {
                isp += istep;
                istep = if j % 2 == 1 { 2 * (naproc - ip) + 1 } else { 2 * ip - 1 };
                if isp <= nspec {
                    let nl = ntloc(1 + (isp - 1) / nth);
                    if nt[ip] + nl <= nttarg {
                        ia[isp] = ip as i64;
                        nt[ip] += nl;
                    } else {
                        ia[isp] = -1;
                    }
                }
            }
        }
        for ip in 1..=naproc {
            if nt[ip] < nttarg {
                for isp in 1..=nspec {
                    if ia[isp] == -1 {
                        let nl = ntloc(1 + (isp - 1) / nth);
                        if nt[ip] + nl <= nttarg {
                            ia[isp] = ip as i64;
                            nt[ip] += nl;
                        }
                    }
                }
            }
        }
        if ia[1..].iter().all(|&x| x > 0) {
            break;
        }
        nttarg += 1;
        if nttarg >= nttmax {
            break;
        }
    }
    assert!(ia[1..].iter().all(|&x| x > 0), "IAPPRO: a spectral component has no rank (w3initmd.F90, label 829)");
    ia[1..].iter().map(|&x| x as usize - 1).collect()
}
