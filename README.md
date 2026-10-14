# kubectlUtil

Aplicativo desktop em Rust + `eframe/egui` para consultas sob demanda a Kubernetes, kubectl e AWS CLI.

## Organização do código

A estrutura foi mantida intencionalmente simples:

```text
src/
├── main.rs          # Inicialização da janela e tema
├── command.rs       # Execução de processos externos (kubectl/AWS CLI)
└── app/
    ├── mod.rs       # Estado do aplicativo e ações/ operações
    ├── ui.rs        # Interface egui (abas, formulários e painéis)
    └── mock.rs      # Resultados simulados para o modo Mock
```

**Onde fazer alterações:**
- Para mudar botões, layout, abas e apresentação: `src/app/ui.rs`.
- Para alterar ações e estado, como cache do Parameter Store e Port Forward: `src/app/mod.rs`.
- Para mudar os dados/cenários simulados: `src/app/mock.rs`.
- Para mudar a forma de executar `kubectl` ou `aws`: `src/command.rs`.

Essa divisão não usa camadas ou abstrações avançadas. Os arquivos continuam dentro de um único crate Rust para facilitar o aprendizado e a compilação.

## Requisitos

- Rust toolchain (`rustup`, `cargo` e `rustc`)
- `kubectl` disponível no `PATH` e configurado para o cluster desejado
- AWS CLI v2 disponível no `PATH` para os recursos AWS/Parameter Store
- Credenciais/perfis AWS e permissões apropriadas, quando usados

## Compilar e executar

```powershell
cargo run
```

Para gerar o executável de release sem abrir uma janela de console no Windows, o `src/main.rs` pode usar o atributo `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` no início do arquivo; depois execute:

```powershell
cargo build --release
```

O executável fica em `target\release\kubectlUtil.exe`.

## Funcionalidades

- Aba **Kubernetes** unifica a seleção de contexto, namespace, Pod e Service com o explorador, diagnóstico e troubleshooting de Pods.
- Consultas de cluster, nodes, eventos, workloads, rede/storage e AWS/EKS sob demanda.
- Explorador de Pods reutiliza a última listagem para diagnóstico local, evitando uma consulta extra quando possível.
- **Port Forward simultâneos**: cada encaminhamento é um processo `kubectl port-forward` independente, iniciado sem janela de console. É possível parar cada encaminhamento individualmente ou parar todos. O aplicativo verifica quando um processo termina e remove-o da lista. Cada encaminhamento precisa de uma porta local exclusiva; portas remotas podem se repetir.
- AWS Systems Manager Parameter Store: lista metadados e mantém cache em memória por perfil, região e caminho. `SecureString` não é descriptografado ao listar; o valor só é solicitado quando o usuário pede explicitamente. O valor revelado não é incluído no histórico geral.
- Histórico local de operações, com limite de 200 entradas.

## Port Forward: uso

1. Na aba **Kubernetes**, selecione contexto e namespace; carregue a lista de Pods e/ou Services e escolha o recurso.
2. Informe a porta local e a porta remota do recurso.
3. Clique em **Iniciar Port Forward do Pod** ou **Iniciar Port Forward do Service**.
4. Para iniciar outro, altere a **porta local** para uma porta livre, selecione outro recurso e inicie novamente.
5. Use o botão **Parar #N** para encerrar um encaminhamento específico ou **Parar todos** para encerrar todos os encaminhamentos iniciados pelo aplicativo.

Exemplo: `localhost:15432 → 5432` para um serviço e `localhost:16379 → 6379` para outro. O processo fica ativo enquanto o encaminhamento estiver listado. Ao fechar o aplicativo, os processos que ele iniciou são encerrados.

## Segurança e limites

- As ações de Kubernetes e AWS são consultas ou operações explicitamente solicitadas pela interface; revise os comandos e permissões do ambiente antes de usar.
- O aplicativo não armazena configurações em SQLite.
- O cache é apenas em memória e se perde ao fechar o aplicativo.
- A execução de `kubectl port-forward` depende da conectividade, permissões RBAC, disponibilidade do Pod/Service e de a porta local estar livre.


## Melhorias futuras

- Analisar formas de transformar num app online, acessível via VPN. (deixar para depois)
- tentar fazer o aplicativo o mais genérico possível. Tudo que for específico a gente coloca em um arquivo de configurações. Por exemplo, o mapeamento entre o nome do serviço e o path do parameter store.