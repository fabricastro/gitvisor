<h1 align="center">Gitvisor</h1>

<p align="center">
  Un cliente visual de git para macOS, Windows y Linux.<br>
  Mirá tus ramas, merges e historia como un grafo — no como un muro de <code>git log</code>.
</p>

<p align="center">
  <a href="#licencia"><img alt="Licencia: MIT" src="https://img.shields.io/badge/licencia-MIT-blue.svg"></a>
  <img alt="Estado: inicial" src="https://img.shields.io/badge/estado-inicial-orange.svg">
  <img alt="Hecho con Tauri" src="https://img.shields.io/badge/hecho%20con-Tauri%20v2-24C8DB.svg">
</p>

<p align="center"><b>Español</b> · <a href="README.en.md">English</a></p>

![Gitvisor mostrando un repositorio con cuatro ramas, dos merges y un tag](docs/screenshot.png)

<sub>La captura muestra el fixture determinista de pruebas, así la misma historia se renderiza igual en cualquier máquina.</sub>

---

## Estado: inicial, y honesto al respecto

Hoy Gitvisor **lee** repositorios. Todavía no escribe en ellos.

| | |
|---|---|
| ✅ Funciona | Grafo de commits, ramas, remotos, tags, detalle de commit, estado del directorio de trabajo |
| 🚧 Todavía no | Stage, commit, crear rama, checkout, fetch, pull, push |
| ❌ No planeado | Rebase, cherry-pick, force-push, resolución visual de conflictos — [por qué](#alcance-qué-hace-y-qué-no-hace-gitvisor) |
| 🧪 Verificado en | macOS y Linux, con la suite end-to-end corriendo en ambos. Windows está soportado por el stack pero todavía nadie lo ejecutó |

No lo apuntes a un repositorio que no puedas permitirte perder. No porque escriba
—no lo hace—, sino porque es joven.

## Empezar

Necesitás [Rust](https://rustup.rs), [Node](https://nodejs.org) y [pnpm](https://pnpm.io).

```bash
git clone https://github.com/fabricastro/gitvisor
cd gitvisor
pnpm install

pnpm app          # ejecutar en modo desarrollo
pnpm app:build    # generar un instalador para tu plataforma
```

Abrí un repositorio con **⌘O**, o arrancá directamente en uno:

```bash
gitvisor /ruta/al/repo
```

## Alcance: qué hace y qué no hace Gitvisor

Gitvisor apunta al **uso diario, menos las operaciones que pueden destruir trabajo.**

Leer, hacer stage, commitear, crear ramas y sincronizar están dentro del alcance.
Rebase, cherry-pick, force-push y resolución visual de conflictos quedan afuera
deliberadamente.

El razonamiento es simple: un defecto en algo dentro del alcance muestra píxeles
equivocados o se niega a ejecutar una acción. Un defecto en `rebase` destruye en
silencio la historia de alguien, y todo el valor de git es ser determinista y
auditable. Esa decisión, con su justificación, está registrada en
[`openspec/config.yaml`](openspec/config.yaml).

Fuera de alcance significa *postergado hasta una decisión deliberada*, no *nunca*.
Las ideas esperando su turno viven en [`openspec/backlog.md`](openspec/backlog.md).

## Arquitectura

```
crates/git-core/     Dominio. Lee el repositorio y calcula el layout del grafo.
                     No sabe nada de Tauri, HTTP ni de ninguna UI.
src-tauri/           Shell de escritorio. Comandos delgados sobre git-core. Sin lógica.
src/                 UI en React, organizada por feature.
tools/git-fixtures/  Repositorios deterministas para el harness de pruebas.
```

Todo lo que entiende de git es Rust. Todo lo que entiende de píxeles es
TypeScript. Se encuentran en un conjunto acotado de comandos, y `git-core` no
tiene idea de que existe una UI — que es justamente lo que hace testeable al
dominio sin necesidad de abrir una ventana.

### La parte interesante: asignación de carriles

El problema difícil en un cliente como este no es la ventana. Es decidir **a qué
carril horizontal pertenece cada commit**, para que una rama de larga vida se
dibuje como una línea recta en lugar de zigzaguear por la pantalla.

[`crates/git-core/src/graph.rs`](crates/git-core/src/graph.rs) lo resuelve en dos
pasadas:

1. **Ubicar** cada commit. Cada carril guarda el id del commit que está
   esperando; cuando ese commit llega toma el carril, y su primer padre continúa
   la línea. Los padres adicionales de un merge se ramifican a su propio carril.
2. **Conectar**. Recién ahora, con el carril final de cada commit ya conocido, se
   emiten las aristas.

Hacer ambas cosas en una sola pasada parece más simple y es sutilmente incorrecto:
una arista se emite antes de que se decida el carril de su padre, así que una rama
lateral que reserva el carril primero arrastra a la línea principal de costado por
el resto del grafo. Hay un test de regresión que lleva ese nombre exacto.

Después la UI dibuja filas, nunca el DAG. El texto del commit es DOM virtualizado
para que siga siendo seleccionable; las líneas detrás son un único canvas que se
repinta al hacer scroll.

## Pruebas

```bash
cargo test --workspace       # dominio, layout del grafo, determinismo del fixture
pnpm build                   # typecheck y bundle
pnpm run e2e:build           # compila el binario de e2e con el frontend embebido
pnpm e2e:native:smoke        # ejecuta la app real en WKWebView real
pnpm e2e:native:regressions
pnpm e2e:browser             # el mismo frontend, en Chrome, contra mocks generados

# Reconstruye el fixture determinista y regenera e2e/mocks/*.json a partir de él.
# Ejecutalo después de cambiar cualquier cosa en crates/git-core/src/model.rs —
# CI falla si los mocks commiteados divergen de lo que esto produce.
pnpm run e2e:mocks

# Imprime el grafo calculado como ASCII para cualquier repositorio: la forma más
# rápida de revisar un cambio de layout. Corrélo al lado de
# `git log --graph --oneline --all` y compará.
cargo run -p git-core --example dump -- /ruta/al/repo
```

La suite end-to-end lanza el **binario real** y maneja el **webview real** a
través de WebDriver, así que lo que verifica es lo que recibe el usuario. El
servidor WebDriver embebido queda fuera de todo build que no sea `e2e` mediante
una feature de Cargo, un gate de capabilities en `build.rs` y una guarda
`compile_error!` — activarlo en un build de release **no compila**, en lugar de
publicar una superficie de control remoto.

Los builds de release además pasan por `scripts/release-scan.sh`, que escanea el
bundle publicado buscando cualquier rastro del plugin y verifica **las dos
direcciones**: que esté ausente del artefacto de release y presente en uno
compilado a propósito con e2e. Así, un chequeo que dejó de coincidir en silencio
no puede pasar por accidente.

El modo browser (`pnpm e2e:browser`) ejecuta el mismo frontend en Chrome contra
mocks de `invoke()` generados directamente desde el fixture, usando los mismos
tipos de `git-core` que usa la app — nunca escritos a mano, y verificados por
diff en CI. No necesita compilar Rust ni WebKit, así que es el loop rápido de
iteración; la suite nativa es la autoridad de corrección, porque el modo browser
no puede ver el renderizado real de WebKit, el IPC real ni el sistema de
capabilities.

Los fixtures fijan la identidad del autor, los timestamps, los nombres de rama y
el contenido del árbol, y verifican los OIDs de los commits contra constantes
hardcodeadas, así la misma historia se renderiza idéntica en todos lados. Ojo:
el determinismo termina en los object IDs. La UI muestra fechas *relativas*, así
que el texto renderizado depende de la fecha de hoy. Nunca hagas aserciones sobre él.

## Cómo se construye este proyecto

Los cambios importantes pasan por un flujo dirigido por especificaciones antes de
que se escriba una línea de código. Las propuestas, especificaciones, diseños y
desgloses de tareas están commiteados en [`openspec/`](openspec/) — incluido el
razonamiento que fue **descartado**, y por qué.

Si querés saber por qué algo es como es, ese directorio es la respuesta, y está
deliberadamente dentro del repositorio en lugar de en las notas privadas de alguien.

## Contribuir

Las contribuciones son bienvenidas, especialmente:

- **Verificación en Windows** — el stack lo soporta; todavía nadie lo ejecutó ahí. macOS y Linux ya corren en CI.
- **Operaciones de escritura** — la lista de alcance de más arriba está libre.
- **Casos borde del layout del grafo** — merges octopus, historias muy anchas, clones shallow.

Antes de abrir un pull request:

1. `cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt --all --check`
2. `pnpm build`
3. Si tocaste el grafo, corré el ejemplo `dump` al lado de `git log --graph` y compará.

Para cualquier cosa más grande que un bugfix, abrí un issue primero, así la
decisión de alcance ocurre antes que el código.

Los issues y pull requests pueden abrirse en español o en inglés.

## Hoja de ruta

- [ ] Visor de diff por archivo
- [ ] Stage, unstage y commit desde la UI
- [ ] Crear rama y checkout
- [ ] Fetch, pull y push
- [ ] Búsqueda en la historia
- [ ] Blame y comparación de ramas
- [ ] Tema claro

## Licencia

MIT — ver [LICENSE](LICENSE).
