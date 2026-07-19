# admet-gnn

Computational drug-discovery screening engine — predicts molecular
properties (aqueous solubility, toxicity risk, blood-brain barrier
permeability) plus standard drug-likeness descriptors from a SMILES string,
using RDKit for cheminformatics and a PyTorch Geometric GNN for the learned
properties.

**Status**: early development (Phase 0 — environment setup).

## Setup

```bash
python3 -m venv venv
source venv/bin/activate        # Windows: venv\Scripts\activate
python -m pip install --upgrade pip
pip install -r requirements.txt
```

See `requirements.txt` for notes on installing PyTorch / PyTorch Geometric
for your specific platform (CPU vs. CUDA).

## Proprietary

This repository is closed source. See `LICENSE`.