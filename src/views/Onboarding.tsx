import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type AppInfo, errorText, type Gender, isAdmin, type Profile, TERMS_EXTENSIONS, type TermsInfo, type UserRole } from "../api";
import { TAGLINE, Wordmark } from "../brand";
import { Bubbe } from "../mascot";
import { Button, ErrorNote, Icon, Spinner } from "../ui";

/**
 * First run: who you are and what you use Bubbe for. A seller says what they sell, so the
 * transcript can say who's who and the tips fit. An admin says what to flag in other people's
 * calls, or lets the AI draft those rules.
 */
export default function Onboarding({ info, onDone }: { info: AppInfo; onDone: () => void }) {
  const [step, setStep] = useState<1 | 2>(1);
  const [profile, setProfile] = useState<Profile>({ ...info.profile, role: info.profile.role || "saljare" });
  const admin = isAdmin(profile);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const finish = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.saveProfile(profile);
      onDone();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex h-full flex-col overflow-auto">
      <div className="h-[52px] shrink-0" data-tauri-drag-region />
      <div className="flex flex-1 items-center justify-center px-10 pb-12">
        <div className="w-full max-w-[520px] space-y-7">
          <div className="flex flex-col items-center gap-3 text-center">
            <Bubbe mood={step === 1 ? "hello" : "idle"} size={112} interactive />
            <Wordmark size={28} />
            <p className="text-[13px] text-muted">{TAGLINE}</p>
          </div>

          <div className="content rise space-y-6 rounded-[28px] p-7 shadow-[0_0_0_0.5px_var(--color-line),0_20px_60px_rgba(0,0,0,0.08)]">
            <Steps step={step} />
            {step === 1 ? (
              <>
                <div className="space-y-1">
                  <h1 className="font-display text-[20px] font-semibold tracking-[-0.015em]">Hej! Vad heter du?</h1>
                  <p className="text-[13px] leading-relaxed text-muted">
                    {admin
                      ? "Bubbe granskar samtal åt dig. Ditt namn används inte för säljaren i samtalen."
                      : 'Då står ditt namn i transkriberingen istället för "Talare 1", och AI:n vet vem som är du.'}
                  </p>
                </div>
                <ProfileFields profile={profile} onChange={setProfile} part="you" autoFocus />
                <div className="flex justify-end">
                  <Button variant="primary" size="lg" disabled={!profile.name.trim()} onClick={() => setStep(2)}>
                    Fortsätt
                  </Button>
                </div>
              </>
            ) : (
              <>
                {admin ? (
                  <div className="space-y-1">
                    <h1 className="font-display text-[20px] font-semibold tracking-[-0.015em]">Vad ska Bubbe flagga?</h1>
                    <p className="text-[13px] leading-relaxed text-muted">
                      Skriv era regler för säljsamtal, en per rad. Bubbe flaggar brott mot dem, och tipsar om sådant som
                      reglerna inte täcker. Du kan ändra dem när som helst.
                    </p>
                  </div>
                ) : (
                  <div className="space-y-1">
                    <h1 className="font-display text-[20px] font-semibold tracking-[-0.015em]">Vad säljer du?</h1>
                    <p className="text-[13px] leading-relaxed text-muted">
                      Med lite bakgrund blir säljtipsen och svaren i chatten mer träffsäkra. Du kan ändra det här när som helst.
                    </p>
                  </div>
                )}
                <ProfileFields profile={profile} onChange={setProfile} part={admin ? "rules" : "work"} autoFocus />
                {error && <ErrorNote>{error}</ErrorNote>}
                <div className="flex items-center justify-between">
                  <Button variant="plain" onClick={() => setStep(1)}>
                    Tillbaka
                  </Button>
                  <div className="flex gap-2">
                    {!profile.company && !(admin ? profile.policy : profile.offer) && (
                      <Button variant="plain" onClick={finish} disabled={saving}>
                        Hoppa över
                      </Button>
                    )}
                    <Button variant="primary" size="lg" onClick={finish} disabled={saving}>
                      Klar <Icon.Check />
                    </Button>
                  </div>
                </div>
              </>
            )}
          </div>
          <p className="text-center text-[12px] text-faint">Allt du skriver här sparas bara på din dator.</p>
        </div>
      </div>
    </div>
  );
}

function Steps({ step }: { step: number }) {
  return (
    <div className="flex gap-1.5">
      {[1, 2].map((n) => (
        <span key={n} className={`h-1 flex-1 rounded-full ${n <= step ? "bg-honey" : "bg-sunken"}`} />
      ))}
    </div>
  );
}

const GENDERS: { value: Gender; label: string }[] = [
  { value: "kvinna", label: "Kvinna" },
  { value: "man", label: "Man" },
  { value: "annat", label: "Annat / vill inte säga" },
];

const ROLES: { value: UserRole; title: string; text: string }[] = [
  { value: "saljare", title: "Jag säljer", text: "Säljtips och coachning på mina egna samtal." },
  { value: "admin", title: "Jag granskar samtal", text: "Admin: flagga brott mot våra regler i andras samtal." },
];

/**
 * The profile form, used in onboarding and in Settings. `you` is name, role and gender,
 * `work` a seller's company and offer, `rules` an admin's company and policy, `all` the
 * lot for the current role.
 */
export function ProfileFields({
  profile,
  onChange,
  part = "all",
  autoFocus = false,
}: {
  profile: Profile;
  onChange: (p: Profile) => void;
  part?: "you" | "work" | "rules" | "all";
  autoFocus?: boolean;
}) {
  const set = (k: keyof Profile) => (v: string) => onChange({ ...profile, [k]: v });
  const admin = isAdmin(profile);
  const showYou = part === "you" || part === "all";
  const showWork = part === "work" || (part === "all" && !admin);
  const showRules = part === "rules" || (part === "all" && admin);
  return (
    <div className="space-y-4">
      {showYou && (
        <>
          <Field label="Vad använder du Bubbe till?" as="div">
            <div className="grid grid-cols-2 gap-2">
              {ROLES.map((r) => {
                const on = (profile.role || "saljare") === r.value;
                return (
                  <button
                    key={r.value}
                    type="button"
                    onClick={() => set("role")(r.value)}
                    aria-pressed={on}
                    className={`flex flex-col items-start gap-1 rounded-2xl px-3.5 py-3 text-left transition-colors ${
                      on ? "bg-accent-soft shadow-[0_0_0_1.5px_var(--color-honey-strong)]" : "bg-sunken hover:bg-hover"
                    }`}
                  >
                    <span className={`flex items-center gap-1.5 text-[13.5px] font-semibold ${on ? "text-accent" : "text-ink"}`}>
                      {r.value === "admin" ? <Icon.Shield /> : <Icon.Sparkle />} {r.title}
                    </span>
                    <span className="text-[12px] leading-snug text-muted">{r.text}</span>
                  </button>
                );
              })}
            </div>
          </Field>
          <Field label="Namn">
            <input
              autoFocus={autoFocus}
              value={profile.name}
              onChange={(e) => set("name")(e.target.value)}
              placeholder="Förnamn Efternamn"
              className={inputClass}
            />
          </Field>
          {!admin && (
            <Field label="Kön" as="div" hint="Så att AI:n skriver hon, han eller hen om dig.">
              <div className="flex flex-wrap gap-1.5">
                {GENDERS.map((g) => (
                  <button
                    key={g.value}
                    type="button"
                    onClick={() => set("gender")(g.value)}
                    className={`h-8 rounded-full px-3.5 text-[13px] font-medium transition-colors ${
                      profile.gender === g.value ? "bg-honey text-on-accent" : "bg-sunken text-ink hover:bg-hover"
                    }`}
                  >
                    {g.label}
                  </button>
                ))}
              </div>
            </Field>
          )}
        </>
      )}
      {(showWork || showRules) && (
        <Field label="Företag">
          <input
            autoFocus={autoFocus && part !== "all"}
            value={profile.company}
            onChange={(e) => set("company")(e.target.value)}
            placeholder={admin ? "Företaget vars samtal du granskar" : "Där du jobbar"}
            className={inputClass}
          />
        </Field>
      )}
      {showRules && <PolicyField profile={profile} onChange={onChange} />}
      {showRules && <TermsField />}
      {showWork && (
        <>
          <Field label="Ditt erbjudande" hint="Vad du säljer, till vem, och dina viktigaste argument.">
            <textarea
              value={profile.offer}
              onChange={(e) => set("offer")(e.target.value)}
              rows={4}
              placeholder="T.ex. Vi säljer ett CRM för små hantverksföretag. Kunderna sparar tid på offerter och missar inga leads. Från 400 kr per användare och månad."
              className={`${inputClass} h-auto resize-none py-2 leading-relaxed`}
            />
          </Field>
        </>
      )}
    </div>
  );
}

const inputClass =
  "selectable h-9 w-full rounded-xl bg-sunken px-3 text-[13.5px] outline-none transition-shadow placeholder:text-faint focus:bg-surface focus:shadow-[0_0_0_2px_var(--color-mark)]";

/** The admin's rules, with "Fråga AI" to draft them from a short description. */
function PolicyField({ profile, onChange }: { profile: Profile; onChange: (p: Profile) => void }) {
  const [about, setAbout] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const draft = async () => {
    setBusy(true);
    setError(null);
    try {
      const policy = await api.draftPolicy(about, profile.policy);
      onChange({ ...profile, policy });
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="space-y-3">
      <Field label="Regler att flagga" hint="En regel per rad. Bubbe numrerar dem och hänvisar till numret när något flaggas.">
        <textarea
          value={profile.policy}
          onChange={(e) => onChange({ ...profile, policy: e.target.value })}
          rows={6}
          placeholder={"T.ex.\nPresentera dig och företaget i början av samtalet.\nFråga om samtycke innan samtalet spelas in.\nLova aldrig rabatter som inte är godkända.\nNämn bindningstid och uppsägning innan kunden säger ja."}
          className={`${inputClass} h-auto resize-y py-2 leading-relaxed`}
        />
      </Field>
      <div className="space-y-2 rounded-2xl bg-sunken p-3">
        <div className="flex items-center gap-1.5 text-[12.5px] font-semibold text-ink">
          <Icon.Sparkle /> Fråga AI
        </div>
        <p className="text-[12px] leading-snug text-muted">
          Beskriv kort vad ni säljer och vad som är viktigt, så skriver AI:n {profile.policy.trim() ? "vidare på reglerna" : "reglerna"} åt dig.
        </p>
        <textarea
          value={about}
          onChange={(e) => setAbout(e.target.value)}
          rows={2}
          placeholder="T.ex. Vi säljer elavtal till privatpersoner per telefon. Säljarna får inte pressa, och kunden ska alltid få veta ångerrätten."
          className={`${inputClass} h-auto resize-none bg-surface py-2 leading-relaxed`}
        />
        <div className="flex items-center justify-end gap-2">
          {busy && <span className="text-[12px] text-muted">Skriver regler…</span>}
          <Button size="sm" onClick={draft} disabled={busy || !about.trim()}>
            {busy ? <Spinner /> : <Icon.Sparkle />} Skriv regler åt mig
          </Button>
        </div>
        {error && <ErrorNote>{error}</ErrorNote>}
      </div>
    </div>
  );
}

/** Upload "Allmänna villkor" so reviews can flag what contradicts them. Saved right away. */
function TermsField() {
  const [terms, setTerms] = useState<TermsInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.termsInfo().then(setTerms).catch(() => {});
  }, []);
  const upload = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      title: "Välj allmänna villkor",
      filters: [{ name: "PDF, Word eller text", extensions: TERMS_EXTENSIONS }],
    });
    if (typeof path !== "string") return;
    setBusy(true);
    setError(null);
    try {
      setTerms(await api.importTerms(path));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    await api.removeTerms();
    setTerms(null);
  };
  return (
    <Field label="Allmänna villkor" as="div" hint="Bubbe flaggar när säljaren säger något som strider mot villkoren. Bara texten sparas, på din dator.">
      {terms ? (
        <div className="flex items-center gap-3 rounded-xl bg-sunken px-3 py-2.5">
          <span className="text-accent">
            <Icon.Check />
          </span>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[13px] font-medium">{terms.fileName}</div>
            <div className="text-[11.5px] text-muted">
              {terms.chars.toLocaleString("sv-SE")} tecken
              {terms.truncated && " · för lång, bara början används"}
            </div>
          </div>
          <Button size="sm" variant="plain" onClick={upload} disabled={busy}>
            {busy ? <Spinner /> : "Byt"}
          </Button>
          <Button size="sm" variant="plain" onClick={remove} disabled={busy}>
            Ta bort
          </Button>
        </div>
      ) : (
        <Button onClick={upload} disabled={busy}>
          {busy ? <Spinner /> : <Icon.Plus />} Ladda upp villkor
        </Button>
      )}
      {error && <ErrorNote>{error}</ErrorNote>}
    </Field>
  );
}

function Field({
  label,
  hint,
  as = "label",
  children,
}: {
  label: string;
  hint?: string;
  /** "div" when the field holds several controls (a label may only wrap one). */
  as?: "label" | "div";
  children: React.ReactNode;
}) {
  const Tag = as;
  return (
    <Tag className="block space-y-1.5">
      <span className="block text-[12.5px] font-medium text-muted">{label}</span>
      {children}
      {hint && <span className="block text-[11.5px] text-faint">{hint}</span>}
    </Tag>
  );
}
