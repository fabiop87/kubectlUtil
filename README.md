# kubectlUtil

Aplicativo desktop em Rust + `gpui-kit` 0.8.0 para consultas sob demanda a Kubernetes, kubectl e AWS CLI.

## Interface

Tema escuro inspirado em liquid glass, com superfícies translúcidas, reflexos em degradê,
bordas suaves e navegação lateral. Os componentes nativos de entrada, seleção pesquisável,
botões e console usam [GPUI Kit 0.8.0](https://gpui-kit.com/docs/getting-started/).
O acabamento é desenhado pelo app; não depende de blur ou refração do compositor do sistema.

A área de trabalho tem rolagem independente do console. O modo Mock permite explorar
os fluxos e simular sucesso, erro, timeout, lista vazia e falta de permissões sem rede.
Valores do Parameter Store são ocultados ao trocar de seção ou desativar a janela.

## Organização do código

```text
src/
├── main.rs            # Inicialização da janela e tema
├── action.rs          # enum Action: identifica cada operação (título, timeout, se é segredo)
├── command.rs         # Execução de processos externos: timeout, cancelamento, processos em segundo plano
└── app/
    ├── mod.rs         # Estado do aplicativo e operações (launch, kubectl, aws, port-forward…)
    ├── results.rs     # O que fazer quando um comando termina (listas, histórico, Parameter Store)
    ├── presets.rs     # Catálogo declarativo de consultas kubectl (um botão = uma linha)
    ├── runner.rs      # trait Runner: execução real ou simulada (Mock) pela mesma interface
    ├── mock.rs        # Cenários e dados simulados (match exaustivo sobre Action)
    ├── parameters.rs  # Estado, cache e comandos do Parameter Store
    ├── port_forward.rs# Port-forwards ativos e detecção de processos encerrados
    ├── diagnostics.rs # Diagnóstico local da lista de Pods
    ├── util.rs        # Validação de porta e data/hora UTC
    └── ui/
        ├── mod.rs             # Laço principal, navegação, painel de saída, histórico
        ├── widgets.rs         # Inputs e Selects pesquisáveis do GPUI Kit
        ├── connection.rs      # Aba Conexão: seleção, explorador, troubleshooting, port-forward
        ├── sections.rs        # Visão geral, Cluster, Rede/storage (a partir dos presets)
        ├── aws.rs             # Aba AWS & EKS
        └── parameter_store.rs # Aba Parameter Store
```

**Como adicionar uma funcionalidade:**

1. **Nova consulta kubectl simples** (um botão): acrescente uma linha em `src/app/presets.rs`
   (rótulo, `Action`, o que precisa estar selecionado e os argumentos com `{ns}`/`{pod}`).
2. Se a operação for nova, acrescente uma variante em `Action` (`src/action.rs`). O compilador
   vai apontar o que falta: o título (`title()`) e os dados simulados em `src/app/mock.rs`
   (o `match` não tem curinga de propósito).
3. Se o resultado precisar atualizar listas ou estado, trate a `Action` em `src/app/results.rs`.
4. Para comportamento fora do padrão (formulários, estado próprio), crie o método em
   `src/app/mod.rs` e o painel em `src/app/ui/`.

**Onde fazer alterações:**
- Botões, layout, abas e apresentação: `src/app/ui/`.
- Consultas kubectl em formato de botão: `src/app/presets.rs`.
- Ações e estado (Port Forward, Parameter Store, contextos): `src/app/mod.rs`.
- Tratamento do resultado de cada operação: `src/app/results.rs`.
- Dados e cenários simulados: `src/app/mock.rs`.
- Como `kubectl`/`aws` são executados (timeout, cancelamento): `src/command.rs`.

Os arquivos continuam dentro de um único crate Rust, sem camadas além das necessárias.

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
- **Contexto explícito**: toda consulta `kubectl` recebe `--context <contexto selecionado>`, então o que a tela mostra é exatamente o que é consultado, sem depender nem alterar o kubeconfig global. Trocar de contexto limpa namespace, Pod e Service (pertencem ao contexto anterior). O botão **Gravar no kubeconfig** executa `kubectl config use-context` apenas se você quiser mudar o padrão dos seus outros terminais.
- **Timeout e cancelamento**: comandos têm limite de tempo (60 s; 5 min para o login SSO) e as consultas ao cluster usam `--request-timeout=30s`. O botão **Cancelar** no painel de saída interrompe a operação em andamento.
- **Login AWS SSO** pode ser feito pelo botão na aba AWS & EKS (abre o navegador e aguarda a autorização) ou copiando o comando para um terminal.
- Consultas de cluster, nodes, eventos, workloads, rede/storage e AWS/EKS sob demanda.
- Explorador de Pods reutiliza a última listagem para diagnóstico local, evitando uma consulta extra quando possível.
- **Port Forward simultâneos**: cada encaminhamento é um processo `kubectl port-forward` independente, iniciado sem janela de console. É possível parar cada encaminhamento individualmente ou parar todos. O aplicativo verifica a cada instante se o processo terminou (Pod reiniciado, porta ocupada, rede caiu…): ele é removido da lista e o motivo (stderr do kubectl) aparece no painel de saída. Cada encaminhamento precisa de uma porta local exclusiva; portas remotas podem se repetir.
- AWS Systems Manager Parameter Store: lista metadados e mantém cache em memória por perfil, região e caminho. `SecureString` não é descriptografado ao listar; o valor só é solicitado quando o usuário pede explicitamente. O valor revelado não é incluído no histórico geral.
- Histórico local de operações, com limite de 200 entradas e data/hora em UTC.
- A saída pode ser salva em `kubectlUtil-output-AAAAMMDDHHMMSS.log` (diretório atual); o texto do painel de saída é selecionável.

## Port Forward: uso

1. Na aba **Kubernetes**, selecione contexto e namespace; carregue a lista de Pods e/ou Services e escolha o recurso.
2. Informe a porta local e a porta remota do recurso.
3. Clique em **Iniciar Port Forward do Pod** ou **Iniciar Port Forward do Service**.
4. Para iniciar outro, altere a **porta local** para uma porta livre, selecione outro recurso e inicie novamente.
5. Use o botão **Parar #N** para encerrar um encaminhamento específico ou **Parar todos** para encerrar todos os encaminhamentos iniciados pelo aplicativo.

Exemplo: `localhost:15432 → 5432` para um serviço e `localhost:16379 → 6379` para outro. O processo fica ativo enquanto o encaminhamento estiver listado. Ao fechar o aplicativo, os processos que ele iniciou são encerrados.

## Testes

```powershell
cargo test
```

Cobrem as partes sem interface: montagem de argumentos e presets, diagnóstico de Pods,
interpretação do JSON do Parameter Store e cache, formatação de data, dados do mock, execução
com timeout/cancelamento e detecção de port-forward encerrado (esses últimos usam `sh` e rodam
apenas em Linux/macOS).

Antes de enviar mudanças: `cargo fmt` e `cargo clippy`.

## Segurança e limites

- As ações de Kubernetes e AWS são consultas ou operações explicitamente solicitadas pela interface; revise os comandos e permissões do ambiente antes de usar.
- O aplicativo não armazena configurações em SQLite.
- O cache é apenas em memória e se perde ao fechar o aplicativo.
- A execução de `kubectl port-forward` depende da conectividade, permissões RBAC, disponibilidade do Pod/Service e de a porta local estar livre.


## Melhorias futuras

- Analisar formas de transformar num app online, acessível via VPN. (deixar para depois)
- tentar fazer o aplicativo o mais genérico possível. Tudo que for específico a gente coloca em um arquivo de configurações. Por exemplo, o mapeamento entre o nome do serviço e o path do parameter store.
