# Macleaner

Utilitário nativo e leve para a barra de menus do macOS (menu bar only) focado em diagnosticar e limpar com segurança **Dados do Sistema** (*System Data*), caches pesados de desenvolvimento e pastas temporárias de aplicativos.

Construído com **Tauri v2**, **Rust** (`jwalk`, `rayon`, `trash`, `window-vibrancy`) e **TypeScript + Vite** sem frameworks pesados.

---

## Principais Recursos

- **Nativo na barra superior do macOS:** roda com `ActivationPolicy::Accessory` (sem ícone no Dock), janela popover sem bordas (`380x480px`) com blur translúcido real via `NSVisualEffectMaterial::Popover` (`window-vibrancy`), suporte automático a **Light / Dark Mode** e fechamento ao perder foco.
- **Interface enxuta em 3 abas:**
  - **Resumo:** saúde real do volume (`Macintosh HD`), barra proporcional entre espaço em uso e espaço mapeado, scan em paralelo com progresso em tempo real e acesso rápido aos maiores ocupantes.
  - **Itens:** pastas ordenadas por tamanho com filtro por risco (`Baixo`, `Médio`, `Alto`), chips das principais extensões, explicação objetiva e visualização expansível de **subpastas reais** (ex.: separando `steamapps/common` de `steamapps/shadercache` e `appcache`).
  - **Ajustes:** seletor de idioma (**Português**, **English**, **Español**), tamanho mínimo de pasta (MB) e motor de IA (Ollama local ou API HTTPS compatível com OpenAI).
- **Varredura rápida e segura:**
  - Travessia paralela com `jwalk` + `rayon` em `~/Library/Caches`, `~/Library/Application Support`, `~/Library/Developer`, `~/.cache` e caches diretos (`.cargo/registry`, `.npm`, `.ollama/models`, `Library/pnpm/store`, `Library/Logs`).
  - Classificação instantânea via regras embutidas (`src-tauri/rules/known.toml`).
  - Bloqueio preventivo de caminhos protegidos pelo **SIP** e **TCC** do macOS, ignorando erros `PermissionDenied` silenciosamente.
- **Remoção 100% reversível:** nunca usa `rm -rf` ou `remove_dir_all`. Todo item ou subpasta é enviado exclusivamente para a **Lixeira nativa do macOS** via crate `trash`.
- **IA híbrida executada apenas no scan:** analisa somente pastas sem regra conhecida usando evidências reais (subpastas, tamanhos, contagem de arquivos, idade e extensões), com `think: false` e descarregamento imediato da RAM (`keep_alive: 0`) no Ollama ao final do scan, ou fallback conservador quando offline.
- **Verificação de atualizações:** botão **Update** no cabeçalho consulta as *Releases* do repositório no GitHub comparando a versão semântica instalada (`v0.1.0`) com versões futuras.

---

## Requisitos para Rodar e Compilar

### Para usuários (executando o `.app` / `.dmg`)
- **macOS 12+** (Apple Silicon `aarch64` ou Intel `x86_64`; efeito visual aprimorado em versões recentes com `NSVisualEffectMaterial::Popover`).
- *(Opcional)* **Ollama** instalado e em execução em `http://127.0.0.1:11434` com pelo menos um modelo local (ex.: `ollama pull qwen3:8b` ou `llama3.2:3b`), ou uma chave de API HTTPS compatível com OpenAI caso queira análise por IA de pastas desconhecidas. Sem IA configurada, o app opera normalmente com regras locais e classificação conservadora.

### Para desenvolvimento / compilação a partir do código-fonte
1. **macOS** com **Xcode Command Line Tools**:
   ```bash
   xcode-select --install
   ```
2. **Rust** (`1.90+`, testado em `1.100.0`) via [rustup](https://rustup.rs/):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
3. **Node.js** (`20+`) e `npm`.

---

## Como Executar

### 1. Instalar dependências
```bash
git clone https://github.com/vicode360/macleaner.git
cd macleaner
npm install
```

### 2. Modo de desenvolvimento
```bash
npm run tauri dev
```
O ícone do **Macleaner** aparecerá na barra superior do macOS. Clique com o botão esquerdo para abrir o popover ou com o botão direito para encerrar (**Quit Macleaner**).

### 3. Gerar pacote de produção (`.app` e `.dmg`)
```bash
npm run tauri build
```
Os artefatos gerados ficam em `src-tauri/target/release/bundle/`.

### 4. Checagens e testes
```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

---

## Arquitetura do Projeto

```text
macleaner/
├── index.html                  # Shell HTML do popover (380x480)
├── src/
│   ├── main.ts                 # UI nativa (3 abas), ícones Lucide e chamadas IPC
│   ├── i18n.ts                 # Traduções PT-BR / EN / ES e formatação Intl
│   ├── styles.css              # Tokens Aqua HIG (Light/Dark) e popover translúcido
│   └── types.ts                # Contratos TypeScript 1:1 com structs Rust Serde
└── src-tauri/
    ├── rules/known.toml        # Regras determinísticas macOS + proteção SIP/TCC
    ├── capabilities/default.json
    ├── tauri.conf.json
    └── src/
        ├── lib.rs              # ActivationPolicy::Accessory, TrayIcon, Positioner e Vibrancy
        ├── rules.rs            # Engine de regras (include_str!) e filtro SIP/TCC
        ├── scanner.rs          # Varredura paralela (jwalk + rayon), subpastas, sysinfo e trash
        ├── ai.rs               # Cliente Ollama loopback / API HTTPS com fallback conservador
        └── commands.rs         # Comandos IPC Tauri, persistência e checagem de updates
```
