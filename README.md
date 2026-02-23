# Akash deploy: Strand Rust Coder 14B

SDL file: `deploy.yaml`

## 1) Prereqs
- Akash CLI installed
- Wallet funded with AKT
- Cert created

## 2) Deploy
```bash
cd akash-strand-rust-coder
provider-services tx deployment create deploy.yaml --from <wallet> --node https://rpc.akashnet.net:443 --chain-id akashnet-2 --gas auto --gas-adjustment 1.4 --yes
provider-services query market bid list --owner $(provider-services keys show <wallet> -a) --node https://rpc.akashnet.net:443 --chain-id akashnet-2
provider-services tx market lease create --dseq <DSEQ> --gseq 1 --oseq 1 --provider <PROVIDER> --from <wallet> --node https://rpc.akashnet.net:443 --chain-id akashnet-2 --gas auto --gas-adjustment 1.4 --yes
provider-services send-manifest deploy.yaml --dseq <DSEQ> --from <wallet> --provider <PROVIDER>
```

## 3) Get endpoint
```bash
provider-services lease-status --dseq <DSEQ> --gseq 1 --oseq 1 --provider <PROVIDER>
```
Use the returned URI as your OpenAI-compatible base URL.

## 4) Hook into Agent Zero
Set your Agent Zero model provider to OpenAI-compatible endpoint:
- Base URL: `http://<akash-uri>/v1`
- Model: `Fortytwo-Network/Strand-Rust-Coder-14B-v1`
- API key: any placeholder value if your client requires one

## 5) Test rust code generation
```bash
curl http://<akash-uri>/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "Fortytwo-Network/Strand-Rust-Coder-14B-v1",
    "messages": [
      {"role": "system", "content": "You are a strict Rust coding assistant."},
      {"role": "user", "content": "Write a Rust function that parses CSV into a Vec<Struct> with error handling and tests."}
    ],
    "temperature": 0.2
  }'
```

## Notes
- A100 is chosen for first-pass reliability for a 14B coder model.
- You can reduce cost later by testing L40S/4090-class offers and tuning max model length.
