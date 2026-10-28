import { useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type AppInfo, type Profile, type CliInfo, errorText, PROVIDER_LABEL, PROVIDER_PHRASE, PROVIDER_VENDOR, type ProviderChoice } from "../api";
import { Button, Card, ErrorNote, Icon, Segmented, Spinner, Toggle } from "../ui";
import { ProfileFields } from "./Onboarding";
import ModelPicker from "./ModelPicker";

const isMac = navigator.userAgent.includes("Mac");
const keychainName = isMac ? "macOS nyckelring" : "Windows Autentiseringshanteraren";

export default function SettingsView({
  info,
  busy,
  onChanged,
  onDataDeleted,
}: {
  info: AppInfo;
  busy: boolean;
  onChanged: () => void;
  onDataDeleted: () => void;
}) {
  return (
    <div className="h-full overflow-auto">
      <div className="h-[52px]" data-tauri-drag-region />
      <div className="mx-auto max-w-[680px] space-y-9 px-8 pb-12">
        <h1 className="font-display text-[26px] font-semibold tracking-[-0.02em]">Inställningar</h1>

        <Group title="Utseende">
          <Card className="flex items-center justify-between gap-4 px-4 py-3">
            <span className="text-[13px] font-medium">Tema</span>
            <Segmented
              value={info.settings.theme}
              onChange={async (t) => {
                await api.setTheme(t);
                onChanged();
              }}
              options={[
                { value: "system", label: "Som datorn" },
                { value: "light", label: "Ljust" },
                { value: "dark", label: "Mörkt" },
              ]}
            />
          </Card>
        </Group>

        <Group title="Om dig" description="Används för att visa vem som är vem och för att ge träffsäkrare tips.">
          <ProfileCard info={info} onSaved={onChanged} />
        </Group>

        <Group title="Talmodell" description="Modellen som skriver ut samtalen. Den körs på din dator.">
          <ModelPicker info={info} onChanged={onChanged} compact busy={busy} />
          {info.gpuCompiled && (
            <Card className="p-4">
              <Toggle
                checked={info.settings.useGpu}
                onChange={async (v) => {
                  await api.updateSettings({ useGpu: v });
                  onChanged();
                }}
                disabled={busy}
                label="Använd grafikkortet"
                hint={
                  info.hardware.gpuName
                    ? `${info.hardware.gpuName}. Stäng av om transkriberingen kraschar eller ger konstiga resultat.`
                    : "Snabbare transkribering. Stäng av om transkriberingen kraschar eller ger konstiga resultat."
                }
              />
            </Card>
          )}
        </Group>

        <Group
          title="AI för säljtips och chatt"
          description="AI:n läser transkriberingen, tar fram säljtips och svarar på dina frågor om samtalet."
        >
          <ProviderCard info={info} onChanged={onChanged} />
          <ApiKeyCard info={info} onChanged={onChanged} />
          <Card className="space-y-4 p-4">
            <Toggle
              checked={info.settings.autoTips}
              onChange={async (v) => {
                await api.updateSettings({ autoTips: v });
                onChanged();
              }}
              label="Ta fram säljtips automatiskt"
              hint="Skickar transkriberingen till AI:n direkt när den är klar."
            />
            <ModelField value={info.settings.claudeModel} onSaved={onChanged} />
          </Card>
        </Group>

        <Group title="Integritet">
          <Card className="space-y-3 p-4 text-[13px] leading-relaxed">
            <p>
              <strong>Ljudet lämnar aldrig din dator.</strong> Säljtipset läser ljudfilen där den ligger och gör ingen kopia av
              den. Transkriberingen sker helt på din dator.
            </p>
            <p>Appen gör bara två saker över internet:</p>
            <ul className="list-disc space-y-1 pl-5">
              <li>
                Laddar ner talmodellen från Hugging Face (KBLab, Kungliga biblioteket). Det sker när du väljer en modell, och
                då skickas inget om dig eller dina samtal.
              </li>
              <li>
                Skickar <strong>texten</strong> från transkriberingen till AI-tjänsten när du tar fram säljtips eller chattar
                om samtalet. Inget ljud skickas. Med API-nyckel eller Claude-prenumeration går texten till Anthropic, med
                ChatGPT-prenumeration till OpenAI.
                {info.activeProvider && ` Just nu används ${PROVIDER_PHRASE[info.activeProvider]}, alltså ${PROVIDER_VENDOR[info.activeProvider]}.`}
              </li>
            </ul>
            <p>
              Med en prenumeration körs Claude Code eller Codex på din dator i en tom mapp, utan tillgång till dina filer eller
              verktyg. Transkriberingar, säljtips och chattar sparas bara i en databas på din dator. API-nyckeln sparas i {keychainName}, inte i
              en vanlig fil.
            </p>
            <p className="text-[12px] text-muted selectable">Datamapp: {info.dataDir}</p>
          </Card>
          <DeleteAll onDeleted={onDataDeleted} />
        </Group>

        <Group title="Uppdateringar">
          <UpdateCard info={info} />
        </Group>

        <p className="text-center text-[12px] text-muted">Säljtipset {info.version}</p>
      </div>
    </div>
  );
}

function Group({ title, description, children }: { title: string; description?: string; children: React.ReactNode }) {
  return (
    <section className="space-y-3">
      <div className="px-1">
        <h2 className="text-[15px] font-semibold tracking-[-0.01em]">{title}</h2>
        {description && <p className="text-[12.5px] text-muted">{description}</p>}
      </div>
      {children}
    </section>
  );
}

function UpdateCard({ info }: { info: AppInfo }) {
  const [state, setState] = useState<
    { kind: "idle" } | { kind: "checking" } | { kind: "latest" } | { kind: "found"; version: string } | { kind: "installing" } | { kind: "error"; text: string }
  >({ kind: "idle" });
  const check = async () => {
    setState({ kind: "checking" });
    try {
      const v = await api.checkUpdate();
      setState(v ? { kind: "found", version: v } : { kind: "latest" });
    } catch (e) {
      setState({ kind: "error", text: errorText(e) });
    }
  };
  const install = async () => {
    setState({ kind: "installing" });
    try {
      await api.installUpdate(); // restarts the app on success
    } catch (e) {
      setState({ kind: "error", text: errorText(e) });
    }
  };
  if (!info.updaterEnabled) {
    return (
      <Card className="px-4 py-3 text-[12.5px] text-muted">
        Version {info.version}. Den här versionen är byggd utan automatiska uppdateringar.
      </Card>
    );
  }
  return (
    <Card className="flex items-center justify-between gap-4 px-4 py-3">
      <div>
        <div className="text-[13px] font-medium">Version {info.version}</div>
        <div className={`text-[12px] ${state.kind === "error" ? "text-danger" : state.kind === "found" ? "text-accent" : "text-muted"}`}>
          {state.kind === "idle" && "Säljtipset letar efter nya versioner när appen startar."}
          {state.kind === "checking" && "Letar…"}
          {state.kind === "latest" && "Du har den senaste versionen."}
          {state.kind === "found" && `Version ${state.version} finns.`}
          {state.kind === "installing" && "Laddar ner och installerar. Appen startar om."}
          {state.kind === "error" && state.text}
        </div>
      </div>
      {state.kind === "found" || state.kind === "installing" ? (
        <Button variant="primary" onClick={install} disabled={state.kind === "installing"}>
          {state.kind === "installing" ? <Spinner /> : "Uppdatera nu"}
        </Button>
      ) : (
        <Button onClick={check} disabled={state.kind === "checking"}>
          {state.kind === "checking" ? <Spinner /> : "Sök efter uppdateringar"}
        </Button>
      )}
    </Card>
  );
}

function ProfileCard({ info, onSaved }: { info: AppInfo; onSaved: () => void }) {
  const [profile, setProfile] = useState<Profile>(info.profile);
  const [status, setStatus] = useState<string | null>(null);
  useEffect(() => setProfile(info.profile), [info.profile]);
  const dirty = JSON.stringify(profile) !== JSON.stringify(info.profile);
  return (
    <Card className="space-y-4 p-4">
      <ProfileFields profile={profile} onChange={setProfile} />
      <div className="flex items-center justify-end gap-3">
        {status && <span className="text-[12px] text-accent">{status}</span>}
        <Button
          variant="primary"
          disabled={!dirty || !profile.name.trim()}
          onClick={async () => {
            await api.saveProfile(profile);
            setStatus("Sparat");
            setTimeout(() => setStatus(null), 1500);
            onSaved();
          }}
        >
          Spara
        </Button>
      </div>
    </Card>
  );
}

function ProviderCard({ info, onChanged }: { info: AppInfo; onChanged: () => void }) {
  const [searching, setSearching] = useState(false);
  const choose = async (p: ProviderChoice) => {
    await api.updateSettings({ aiProvider: p });
    onChanged();
  };
  const search = async () => {
    setSearching(true);
    try {
      await api.refreshAi();
      onChanged();
    } finally {
      setSearching(false);
    }
  };
  const cliStatus = (cli: CliInfo | null, name: string, login: string, install: string) => {
    if (!cli) return { ok: false, text: `Hittades inte på datorn. ${install}` };
    if (!cli.loggedIn) return { ok: false, text: `${name} finns men är inte inloggat. Kör ${login} i Terminalen.` };
    return { ok: true, text: `${cli.detail ?? "Inloggad"} · via ${name}` };
  };
  const options: { id: ProviderChoice; label: string; status: { ok: boolean; text: string } }[] = [
    {
      id: "auto",
      label: "Automatiskt",
      status: {
        ok: info.activeProvider !== null,
        text: info.activeProvider
          ? `Använder ${PROVIDER_PHRASE[info.activeProvider]}. Ordning: API-nyckel, Claude, ChatGPT.`
          : "Ingen AI hittades än.",
      },
    },
    {
      id: "claude-code",
      label: PROVIDER_LABEL["claude-code"],
      status: cliStatus(info.detected.claudeCode, "Claude Code", "`claude`", "Installera Claude Code och logga in med ditt Claude-konto."),
    },
    {
      id: "codex",
      label: PROVIDER_LABEL.codex,
      status: cliStatus(info.detected.codex, "Codex", "`codex login`", "Installera Codex CLI och logga in med ditt ChatGPT-konto."),
    },
    {
      id: "api",
      label: PROVIDER_LABEL.api,
      status: { ok: info.hasApiKey, text: info.hasApiKey ? "Nyckel sparad" : "Lägg till en nyckel nedan." },
    },
  ];
  return (
    <Card className="divide-y divide-line">
      {options.map((o) => (
        <label key={o.id} className="flex items-start gap-3 px-4 py-3">
          <input type="radio" name="ai-provider" checked={info.aiProvider === o.id} onChange={() => choose(o.id)} className="peer sr-only" />
          <span
            className={`mt-0.5 flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full border-[1.5px] ${
              info.aiProvider === o.id ? "border-accent bg-accent text-on-accent" : "border-black/20 dark:border-white/25"
            }`}
          >
            {info.aiProvider === o.id && <Icon.Check />}
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-[13px] font-medium">{o.label}</span>
            <span className={`block text-[12px] ${o.status.ok ? "text-accent" : "text-muted"}`}>{o.status.text}</span>
          </span>
        </label>
      ))}
      <div className="flex items-center justify-between gap-3 px-4 py-3">
        <span className="text-[12px] text-muted">
          Har du installerat eller loggat in i Claude Code eller Codex nyss? Sök igen.
        </span>
        <Button onClick={search} disabled={searching}>
          {searching ? <Spinner /> : "Sök igen"}
        </Button>
      </div>
    </Card>
  );
}

function ApiKeyCard({ info, onChanged }: { info: AppInfo; onChanged: () => void }) {
  const [key, setKey] = useState("");
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const [working, setWorking] = useState<"save" | "test" | null>(null);

  const saveKey = async () => {
    setStatus(null);
    setWorking("save");
    try {
      await api.testApiKey(key);
      await api.saveApiKey(key);
      setKey("");
      setStatus({ ok: true, text: `Nyckeln fungerar och är sparad i ${keychainName}.` });
      onChanged();
    } catch (e) {
      setStatus({ ok: false, text: errorText(e) });
    } finally {
      setWorking(null);
    }
  };

  const test = async () => {
    setStatus(null);
    setWorking("test");
    try {
      await api.testApiKey(key || undefined);
      setStatus({ ok: true, text: "Nyckeln fungerar." });
    } catch (e) {
      setStatus({ ok: false, text: errorText(e) });
    } finally {
      setWorking(null);
    }
  };

  const remove = async () => {
    await api.removeApiKey();
    setStatus(null);
    onChanged();
  };

  return (
    <Card className="space-y-3 p-4">
      <div className="flex items-center justify-between">
        <label htmlFor="apikey" className="text-[13px] font-medium">
          Anthropic API-nyckel
        </label>
        <span className={`text-[12px] ${info.hasApiKey ? "text-accent" : "text-muted"}`}>
          {info.hasApiKey ? "✓ Sparad" : "Ingen nyckel sparad"}
        </span>
      </div>
      <div className="flex gap-2">
        <input
          id="apikey"
          type="password"
          autoComplete="off"
          spellCheck={false}
          placeholder={info.hasApiKey ? "Klistra in en ny nyckel för att byta" : "sk-ant-…"}
          value={key}
          onChange={(e) => setKey(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && key && saveKey()}
          className="min-w-0 flex-1 rounded-lg border border-line bg-bg px-3 py-1.5 font-mono text-[13px] outline-none focus:border-accent selectable"
        />
        <Button variant="primary" onClick={saveKey} disabled={!key || working !== null}>
          {working === "save" ? <Spinner /> : "Spara"}
        </Button>
        <Button onClick={test} disabled={(!key && !info.hasApiKey) || working !== null}>
          {working === "test" ? <Spinner /> : "Testa nyckeln"}
        </Button>
        {info.hasApiKey && (
          <Button variant="plain" onClick={remove}>
            Ta bort
          </Button>
        )}
      </div>
      {status && (
        <p className={`text-[12.5px] ${status.ok ? "text-accent" : "text-danger"}`}>{status.text}</p>
      )}
      <p className="text-[12px] text-muted">
        Varje säljare använder sin egen nyckel. Skapa en på console.anthropic.com under API Keys. Nyckeln sparas i{" "}
        {keychainName}.
      </p>
    </Card>
  );
}

function ModelField({ value, onSaved }: { value: string; onSaved: () => void }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const dirty = draft.trim() !== value;
  return (
    <div className="space-y-1">
      <label htmlFor="claudemodel" className="text-[13px] font-medium">
        Claude-modell
      </label>
      <div className="flex gap-2">
        <input
          id="claudemodel"
          value={draft}
          spellCheck={false}
          onChange={(e) => setDraft(e.target.value)}
          className="min-w-0 flex-1 rounded-lg border border-line bg-bg px-3 py-1.5 font-mono text-[13px] outline-none focus:border-accent selectable"
        />
        <Button
          disabled={!dirty}
          onClick={async () => {
            await api.updateSettings({ claudeModel: draft.trim() });
            onSaved();
          }}
        >
          Spara
        </Button>
        <Button
          variant="plain"
          disabled={value === "claude-sonnet-5-5"}
          onClick={async () => {
            await api.updateSettings({ claudeModel: "" });
            onSaved();
          }}
        >
          Återställ
        </Button>
      </div>
      <p className="text-[12px] text-muted">Standard är claude-sonnet-5-5. Gäller API-nyckel och Claude-prenumeration; Codex använder sin egen standardmodell.</p>
    </div>
  );
}

function DeleteAll({ onDeleted }: { onDeleted: () => void }) {
  const [error, setError] = useState<string | null>(null);
  return (
    <Card className="flex items-center justify-between gap-4 p-4">
      <div>
        <div className="text-[13px] font-medium">Radera all data</div>
        <p className="text-[12px] text-muted">
          Tar bort alla transkriberingar, säljtips, inställningar, nedladdade talmodeller och API-nyckeln. Dina ljudfiler
          påverkas inte.
        </p>
        {error && <ErrorNote>{error}</ErrorNote>}
      </div>
      <Button
        variant="danger"
        className="shrink-0"
        onClick={async () => {
          const ok = await ask("Allt som Säljtipset har sparat tas bort. Det går inte att ångra.", {
            title: "Radera all data?",
            kind: "warning",
            okLabel: "Radera allt",
            cancelLabel: "Avbryt",
          });
          if (!ok) return;
          try {
            await api.deleteAllData();
            onDeleted();
          } catch (e) {
            setError(errorText(e));
          }
        }}
      >
        Radera all data
      </Button>
    </Card>
  );
}
