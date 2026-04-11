# Roadmap dalšího vývoje Palyra

## Globální guardraily pro celou roadmapu

- Zachovat fail-closed výchozí chování, approval model, audit a trust boundaries; žádný milestone je nesmí obcházet.
- Upřednostňovat additive a backward-compatible změny; destruktivní migrace jen s preview, backupem a rollbackem.
- Každý milestone má skončit nejen změnou kódu, ale i testy, dokumentací, diagnostikou a support-bundle pokrytím.
- Pokud milestone zasahuje CLI, web, desktop, proto kontrakty nebo browser/runtime vrstvu, je potřeba udržet parity a regresní pokrytí napříč povrchy.

## Fáze a milestoney

## Fáze 1 – Bezpečný základ obnovy a oprav

Proměnit setup, upgrade a obnovu systému z diagnostiky na říditelnou, auditovatelnou a vratitelnou opravu bez oslabení fail-closed hranic.

- [x] [F01-M01 – Opravný framework pro `palyra doctor`](milestones/F01-M01_doctor-repair-framework.md) – Rozšířit `doctor` z read-only reportu na plánovací a opravný orchestrátor s preview/apply režimy a řízenými zásahy.
- [x] [F01-M02 – Atomické zálohy konfigurace a rollback před každou opravou](milestones/F01-M02_atomic-backup-and-rollback.md) – Před každou mutací vytvářet konzistentní backup manifest a mít jednoduchý návrat na předchozí stav.
- [x] [F01-M03 – Verzované migrace konfigurací a úklid legacy shape](milestones/F01-M03_config-schema-migrations-and-legacy-cleanup.md) – Zavést explicitní schema versioning a bezpečné migrátory místo křehkého ručního přepisování starých shape.
- [x] [F01-M04 – Úklid zastaralého runtime stavu a sirotčích artefaktů](milestones/F01-M04_stale-runtime-and-orphan-cleanup.md) – Detekovat a bezpečně uklízet staré locky, sockety, orphan browser state a další pozůstatky po pádech procesů.
- [x] [F01-M05 – Oprava browser, access, onboarding a pairing stavu](milestones/F01-M05_repair-browser-access-onboarding-and-pairing-state.md) – Zacílit recovery na nejčastější provozní bolesti kolem browser relay, pairing requestů a access registrů.
- [x] [F01-M06 – UX obnovy, support bundle a koncová smoke sada](milestones/F01-M06_recovery-ux-support-bundle-and-smoke-suite.md) – Zabalit novou recovery vrstvu do použitelného UX a ověřit ji na realistických fixture scénářích rozbitých stavů.

## Fáze 2 – Odolnost běhu a samoléčba

Navázat na recovery foundation a doplnit průběžné rozpoznávání incidentů, automatické heal kroky a silnou observabilitu bez oslabení approval modelu.

- [x] [F02-M01 – Jednotná incident taxonomie a runtime stavový model](milestones/F02-M01_incident-taxonomy-and-state-model.md) – Zavést společný slovník incidentů, degradací a healing stavů napříč daemonem, browserem, skills i connector vrstvou.
- [x] [F02-M02 – Detektor zaseknutých runů, session a background tasků](milestones/F02-M02_stuck-run-session-task-detector.md) – Průběžně rozpoznávat běhy a tasky, které se netváří jako fail, ale zjevně se nikam neposouvají.
- [x] [F02-M03 – Samoléčba pro `palyra-browserd` a browser session vrstvu](milestones/F02-M03_browserd-session-auto-healing.md) – Přidat cílenou samoléčbu pro browser daemon, session registry a relay vrstvu.
- [x] [F02-M04 – Karanténa a obnovovací workflow pro rozbité skills, pluginy a tool artefakty](milestones/F02-M04_skill-plugin-tool-quarantine-and-rebuild.md) – Bezpečně vyřazovat nefunkční nebo podezřelé artefakty a nabízet jejich kontrolovaný návrat do provozu.
- [x] [F02-M05 – Remediační politika respektující approvals a eskalační pravidla](milestones/F02-M05_approval-aware-remediation-policies.md) – Rozdělit heal akce podle rizika a navázat je na existující policy a approval model.
- [x] [F02-M06 – Observabilita samoléčby, kill switche a chaos/regresní sada](milestones/F02-M06_healing-observability-kill-switches-and-chaos-suite.md) – Dodat operátorské přepínače, detailní telemetry a testovací harness pro novou self-healing vrstvu.

## Fáze 3 – Kompletní browser vrstva a ladicí nástroje

Doplnit chybějící browser surface, failure artefakty a operátorský debug workflow tak, aby browser capabilities působily produktově dokončeně.

- [x] [F03-M01 – Rozšíření browser kontraktů a generovaných stubů](milestones/F03-M01_browser-contract-expansion.md) – Doplnit protokol a API kontrakty pro `press`, `select`, `highlight`, `console` a `pdf` tak, aby šly bezpečně používat napříč CLI, webem a tool runtime.
- [x] [F03-M02 – Implementace `press`, `select` a `highlight` v browser enginu](milestones/F03-M02_press-select-highlight-engine.md) – Dodat chybějící interakční akce přímo v browserd enginu a gRPC service vrstvě.
- [x] [F03-M03 – Integrace nových browser akcí do tool runtime a policy vrstvy](milestones/F03-M03_browser-tool-runtime-and-policy-surface.md) – Napojit nové browser akce do tool runtime, approvals a policy rozhodování.
- [x] [F03-M04 – Console logy, JS/page diagnostika a strukturované ladicí výstupy](milestones/F03-M04_browser-console-and-page-diagnostics.md) – Zpřístupnit browser console, JS chyby a page varování jako operátorsky použitelný debug artefakt.
- [x] [F03-M05 – PDF export jako standardní browser capability](milestones/F03-M05_browser-pdf-export-pipeline.md) – Dodat bezpečný a auditovatelný PDF export z browser session včetně registrace vzniklého artefaktu.
- [x] [F03-M06 – Failure artefakty a trace bundle pro browser akce](milestones/F03-M06_browser-failure-artifacts-and-trace-bundle.md) – Při browser failu automaticky sesbírat to nejdůležitější pro rychlou diagnózu bez ručního sběru dat.
- [x] [F03-M07 – CLI/Web ladicí workbench a jasné režimy profilů](milestones/F03-M07_browser-cli-web-debug-workbench.md) – Postavit nad novými browser capabilities přehledný operátorský debug workflow a zřetelný model profilů.
- [x] [F03-M08 – Browser regresní matice a deterministické fixture](milestones/F03-M08_browser-regression-matrix-and-deterministic-fixtures.md) – Uzavřít fázi robustní testovací sadou pro nové browser capabilities a diagnostické flow.

## Fáze 4 – Kontinuita kontextu a kompaktace

Proměnit compaction z naznačené stub vrstvy na skutečný, bezpečný a operátorsky viditelný mechanismus zachování dlouhodobého kontextu.

- [x] [F04-M01 – Náhrada compaction stubu za reálný lifecycle kompaktace](milestones/F04-M01_real-compaction-lifecycle.md) – Opustit stávající stub a zavést skutečný compaction orchestrátor s checkpointy a auditními eventy.
- [x] [F04-M02 – Plánovač pre-compaction memory flush](milestones/F04-M02_pre-compaction-memory-flush-planner.md) – Před compactionem explicitně vyhodnotit, co je potřeba uložit jako durable kontext.
- [x] [F04-M03 – Perzistentní writery do workspace a merge semantika](milestones/F04-M03_durable-workspace-writers.md) – Umět planner kandidáty zapisovat do curated dokumentů a dalších durable struktur idempotentně a bez konfliktů.
- [x] [F04-M04 – Filtrace šumu, detekce konfliktů a pravidla bezpečného zápisu](milestones/F04-M04_noise-filtering-and-safe-write-rules.md) – Před zápisem do durable kontextu filtrovat šum, citlivé zbytky a konfliktní tvrzení.
- [x] [F04-M05 – Compaction UI, diffy a auditní workflow](milestones/F04-M05_compaction-ui-diff-audit.md) – Zpřehlednit compaction a jeho dopad přímo ve webu, CLI a auditních površích.
- [x] [F04-M06 – Dlouhosession regresní sada, rollback a quality gate](milestones/F04-M06_long-session-regression-and-quality-gates.md) – Otestovat compaction continuity na dlouhých session a zafixovat minimální kvalitu výstupů i rollback cesty.

## Fáze 5 – Vrstva providerů a modelů a orchestrace více providerů

Přetavit dnešní provider vrstvu do metadata-driven registry s capability matrixí, failoverem a operátorským ovládáním bez narušení usage governance.

- [x] [F05-M01 – Registr providerů a nový datový model providerů/modelů](milestones/F05-M01_provider-registry-and-data-model.md) – Přejít od jednoho úzkého provider modelu k registry providerů, modelů a jejich provozních metadat.
- [x] [F05-M02 – Capability matice pro provider/model](milestones/F05-M02_provider-capability-matrix.md) – Popsat u každého modelu a provideru, co skutečně umí a jaké má provozní vlastnosti.
- [x] [F05-M03 – Normalizace auth profilů a provider credentials](milestones/F05-M03_provider-auth-and-profile-normalization.md) – Oddělit provider auth vrstvu od model selection a připravit ji na multi-provider provoz.
- [x] [F05-M04 – Health probes, model discovery a flow `test connection`](milestones/F05-M04_provider-health-model-discovery-and-test-connection.md) – Dodat operátorské ověření provideru a modelů bez nutnosti čekat na runtime selhání.
- [x] [F05-M05 – Cross-provider failover a retry orchestrace](milestones/F05-M05_cross-provider-failover-and-retry.md) – Při výpadku nebo degradaci umět přepnout na další provider/model bez ztráty kontroly.
- [x] [F05-M06 – Response cache a bezpečný replay](milestones/F05-M06_provider-response-cache-and-safe-replay.md) – Přidat cache vrstvu pro vhodné typy odpovědí a použít ji jako doplněk failoveru i cost governance.
- [x] [F05-M07 – Operátorské UX pro providery, rozpočty a vysvětlení routingu](milestones/F05-M07_provider-ux-budgets-and-routing-explanations.md) – Udělat z provider layer čitelnou součást control plane, ne skrytý interní subsystém.
- [x] [F05-M08 – První skutečný druhý provider a migrační playbook](milestones/F05-M08_first-real-second-provider-and-migration-playbook.md) – Ověřit novou architekturu reálným druhým provider adapterem a dotaženým migration story.

## Fáze 6 – Uzavřená učící smyčka a evoluce paměti

Přidat behaviorální vrstvu, která po běhu reflektuje nové poznatky, vytváří kandidáty do paměti a navrhuje znovupoužitelné postupy bez oslabení trust modelu.

- [x] [F06-M01 – Orchestrátor post-run reflexe](milestones/F06-M01_post-run-reflection-orchestrator.md) – Po dokončení runu spouštět background reflection job, který analyzuje, co se má uchovat nebo zlepšit.
- [x] [F06-M02 – Schéma kandidátů a provenance](milestones/F06-M02_candidate-schema-and-provenance.md) – Zavést jednotný model kandidátů pro durable facts, preference a reusable postupy.
- [x] [F06-M03 – Bezpečný auto-write durable facts](milestones/F06-M03_safe-auto-write-durable-facts.md) – Vybrané nízkorizikové memory facts umět uložit automaticky, pokud splní přísná pravidla.
- [x] [F06-M04 – Model preferencí a uživatelsko-operátorský profil](milestones/F06-M04_preference-model-and-user-operator-profile.md) – Oddělit dlouhodobé preference od faktických memory záznamů a dát jim vlastní lifecycle.
- [x] [F06-M05 – Extrakce reusable procedure kandidátů](milestones/F06-M05_reusable-procedure-candidate-extraction.md) – Z opakovaných úspěšných běhů vytahovat kandidáty na znovupoužitelné postupy a runbooky.
- [x] [F06-M06 – Promoce skill kandidátů přes karanténu, signing a audit](milestones/F06-M06_skill-promotion-via-quarantine-and-signing.md) – Napojit procedure kandidáty na existující trust model tak, aby se z nich mohly stát bezpečné skills.
- [x] [F06-M07 – Revizní UI, prahy a anti-poisoning testy](milestones/F06-M07_learning-review-ui-thresholds-and-poison-tests.md) – Uzavřít learning loop operátorským review workflow a sadou testů proti otravě paměti nebo procedur.

## Fáze 7 – Cíle a produktizace automatizací

Povýšit sessions a routines na srozumitelnější dlouhodobé cíle, Heartbeat režimy a durable automation produkty.

- [x] [F07-M01 – Doménový model pro objective a storage](milestones/F07-M01_objective-domain-model-and-storage.md) – Zavést entitu dlouhodobého objective nad sessions, runy a routines.
- [x] [F07-M02 – Navázání objective na workspace a current focus](milestones/F07-M02_objective-workspace-binding.md) – Propojit objective s curated workspace dokumenty, current focus a session kontextem.
- [x] [F07-M03 – Lifecycle `fire/pause/resume/cancel/archive`](milestones/F07-M03_objective-lifecycle-fire-pause-resume.md) – Dodat nad objectives jasný operátorský lifecycle a spouštěcí logiku.
- [x] [F07-M04 – Success criteria, attempt history a approach log](milestones/F07-M04_objective-success-criteria-and-approach-history.md) – U každého objective udržovat, co znamená úspěch, co už bylo zkuseno a jaký je další pokus.
- [x] [F07-M05 – Heartbeat jako explicitní produktový režim](milestones/F07-M05_heartbeat-as-first-class-mode.md) – Povýšit stávající heartbeat template na první třídu produktu s vlastním UX a lifecyclem.
- [x] [F07-M06 – Standing Orders a Flow/Program vrstva](milestones/F07-M06_standing-orders-and-flow-program-layer.md) – Produktizovat nad routines a objectives srozumitelnější druhy automatizací pro běžné use-case.
- [x] [F07-M07 – UX pro objectives a automatizace napříč webem, CLI a TUI](milestones/F07-M07_objective-and-automation-ux.md) – Uzavřít fázi sjednoceným operátorským povrchem pro objectives, heartbeaty a standing orders.

## Fáze 8 – Profily, vzdálený přístup a interoperabilita

Vytáhnout existující foundations pro profily a ACP do first-class produktu a doplnit kompatibilní MCP fasádu i snadnější verified remote flow.

- [x] [F08-M01 – Plnohodnotný lifecycle CLI profilů](milestones/F08-M01_cli-profile-lifecycle.md) – Přidat explicitní `profile create/list/use/delete/rename/show` místo low-level implicitní práce s profily.
- [x] [F08-M02 – Import/export/clone profilů a namespace izolace](milestones/F08-M02_profile-import-export-clone-and-isolation.md) – Umožnit profily bezpečně přenášet, klonovat a izolovat mezi prostředími.
- [x] [F08-M03 – Bezpečnostní guardraily profilů a environment bannery](milestones/F08-M03_profile-safety-guardrails-and-banners.md) – Snížit riziko omylu mezi prod/staging/dev profily a zvýšit jejich viditelnost v UI i CLI.
- [x] [F08-M04 – Profile-aware startup a switching v desktop companion](milestones/F08-M04_desktop-profile-aware-startup.md) – Dodat do desktopu snadné přepínání profilů a jasnou práci s aktivním profilem.
- [x] [F08-M05 – Sjednocený verified remote dashboard flow](milestones/F08-M05_verified-remote-dashboard-flow.md) – Zjednodušit first-connection remote access bez oslabení cert pinningu a trust modelu.
- [x] [F08-M06 – Úklid ACP bridge a stabilní session binding](milestones/F08-M06_acp-bridge-cleanup-and-stable-binding.md) – Zpevnit stávající ACP bridge tak, aby byl spolehlivým interním i externím integračním bodem.
- [x] [F08-M07 – MCP read-only fasáda](milestones/F08-M07_mcp-read-only-facade.md) – Přidat první MCP server režim pro bezpečné read-only přístupy do sessions, memory a artefaktů.
- [x] [F08-M08 – MCP mutace, approvals a interop playbook](milestones/F08-M08_mcp-mutations-approvals-and-interop-playbook.md) – Rozšířit MCP fasádu o řízené mutace, approvals a dokumentovanou interop story.

## Fáze 9 – Ergonomie UX a parita příkazů

Vyleštit každodenní práci v TUI, webu a chat composeru tak, aby silné jádro Palyry bylo stejně příjemné i v běžném používání.

- [x] [F09-M01 – Sdílený registr slash commandů](milestones/F09-M01_shared-slash-command-registry.md) – Sjednotit slash commandy a jejich metadata mezi webem, TUI a dalšími chat povrchy.
- [x] [F09-M02 – Autocomplete, autosuggest a command palette](milestones/F09-M02_autocomplete-autosuggest-and-command-palette.md) – Přidat rychlé nápovědy a doplňování pro slash commandy, entity a parametry.
- [x] [F09-M03 – `/undo` a obnova z checkpointu](milestones/F09-M03_undo-and-checkpoint-restore.md) – Dodat uživatelsky srozumitelné vrácení posledního kroku a obnovu session stavu tam, kde je to bezpečné.
- [x] [F09-M04 – Interrupt-and-redirect a sjednocená cancel semantics](milestones/F09-M04_interrupt-and-redirect.md) – Umožnit okamžitě přerušit běžící run a plynule navázat novým směrem bez chaosu v session.
- [x] [F09-M05 – Balík pokročilých příkazů a parita napříč povrchy](milestones/F09-M05_power-user-command-pack.md) – Přidat sadu rychlých příkazů pro model, objective, browser a profilové workflow.
- [x] [F09-M06 – UX telemetry, keyboard navigation a help guardraily](milestones/F09-M06_ux-telemetry-keyboard-nav-and-help-guardrails.md) – Uzavřít ergonomickou fázi měřením, klávesnicovou navigací a testovaným help systémem.

## Fáze 10 – Administrativní operace pro Discord

Prohloubit hlavní konektor tak, aby Discord-first provoz dostal skutečně administrativně použitelný surface místo jen základního messagingu.

- [x] [F10-M01 – Protokol a permission model pro administrativní operace Discordu](milestones/F10-M01_discord-admin-protocol-and-permissions.md) – Rozšířit connector protokol o read/search/edit/delete/react operace a jejich permission/approval mapování.
- [x] [F10-M02 – Read/search/history vrstva pro Discord zprávy](milestones/F10-M02_discord-read-search-and-history-surface.md) – Dodat čtení a vyhledávání zpráv včetně práce s historií a artefakty.
- [x] [F10-M03 – Edit/delete/react operace s auditem a approvals](milestones/F10-M03_discord-edit-delete-react-operations.md) – Přidat mutační admin operace na zprávách a reakcích s přísnými guardraily.
- [x] [F10-M04 – UX a regresní sada pro administrativní operace Discordu](milestones/F10-M04_discord-operator-ux-and-regression-suite.md) – Zabalit nové Discord admin capabilities do použitelných povrchů a otestovat je end-to-end.

## Fáze 11 – Vlastní node klient a desktopové capability

Převést existující node infrastrukturu z backendové foundation do skutečně použitelného first-party klienta, který umí plnit omezený, ale reálný native capability set.

- [x] [F11-M01 – Architektura node hostu a capability kontrakt](milestones/F11-M01_node-host-architecture-and-capability-contract.md) – Zafixovat, co přesně znamená first-party node klient a jaké capability kontrakty bude bezpečně obsluhovat.
- [x] [F11-M02 – Desktop pairing a enrollment jako node](milestones/F11-M02_desktop-node-pairing-and-enrollment.md) – Umožnit desktop companion přihlásit a udržovat se jako cert-bound node klient.
- [x] [F11-M03 – Capability dispatch loop a bezpečný lokální runtime](milestones/F11-M03_desktop-capability-dispatch-loop.md) – Přenést capability requesty z daemonu do desktopu a bezpečně vracet jejich výsledky.
- [x] [F11-M04 – Balík native capability v1 pro desktop](milestones/F11-M04_desktop-native-capabilities-v1.md) – Dodat omezený, ale produktově smysluplný balík prvních nativních capability.
- [x] [F11-M05 – Inventory, approvals a handoff vrstva pro node capabilities](milestones/F11-M05_node-inventory-approval-and-handoff-surfaces.md) – Udělat z desktop node capability přehlednou součást control plane a approval workflow.
- [x] [F11-M06 – Rotace, recovery, offline režim a plán mobilní konvergence](milestones/F11-M06_node-rotation-recovery-offline-and-mobile-plan.md) – Uzavřít desktop node fázi životním cyklem zařízení a připravit rozumný most k budoucím mobilním klientům.

## Fáze 12 – Strategické volitelné sázky

Až po dokončení hlavních gapů otevírat širší experimenty a akcelerátory, které mají potenciál, ale nejsou dnes kritickou mezerou.

- [x] [F12-M01 – Produktová vrstva execution backendů](milestones/F12-M01_execution-backends-product-layer.md) – Z existujících sandbox, remote a node stavebnic udělat srozumitelnější execution backend matrix.
- [ ] [F12-M02 – Dynamic tool builder pod karanténou](milestones/F12-M02_dynamic-tool-builder-under-quarantine.md) – Opatrně otevřít možnost generovat nové tool/skill kandidáty přes builder loop, ale jen pod tvrdými guardraily.
- [ ] [F12-M03 – Voice/TTS interakční vrstva](milestones/F12-M03_voice-and-tts-layer.md) – Navázat na existující audio ingest a přidat řízenou voice/TTS user-facing vrstvu.
- [ ] [F12-M04 – Native canvas/A2UI ambient experimenty a governance](milestones/F12-M04_native-canvas-and-ambient-governance.md) – Rozvíjet dál A2UI a případné ambient/native surface jen pod přísnou produktovou a bezpečnostní disciplínou.
