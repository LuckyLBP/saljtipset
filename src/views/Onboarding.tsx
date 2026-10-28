import { useState } from "react";
import { api, type AppInfo, errorText, type Gender, type Profile } from "../api";
import { Mark, TAGLINE, Wordmark } from "../brand";
import { Button, ErrorNote, Icon } from "../ui";

/** First run: who you are, so the transcript can say who's who and the AI knows what you sell. */
export default function Onboarding({ info, onDone }: { info: AppInfo; onDone: () => void }) {
  const [step, setStep] = useState<1 | 2>(1);
  const [profile, setProfile] = useState<Profile>(info.profile);
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
            <Mark size={64} />
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
                    Då står ditt namn i transkriberingen istället för "Talare 1", och AI:n vet vem som är du.
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
                <div className="space-y-1">
                  <h1 className="font-display text-[20px] font-semibold tracking-[-0.015em]">Vad säljer du?</h1>
                  <p className="text-[13px] leading-relaxed text-muted">
                    Med lite bakgrund blir säljtipsen och svaren i chatten mer träffsäkra. Du kan ändra det här när som helst.
                  </p>
                </div>
                <ProfileFields profile={profile} onChange={setProfile} part="work" autoFocus />
                {error && <ErrorNote>{error}</ErrorNote>}
                <div className="flex items-center justify-between">
                  <Button variant="plain" onClick={() => setStep(1)}>
                    Tillbaka
                  </Button>
                  <div className="flex gap-2">
                    {!profile.company && !profile.offer && (
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
        <span key={n} className={`h-1 flex-1 rounded-full ${n <= step ? "bg-accent" : "bg-sunken"}`} />
      ))}
    </div>
  );
}

const GENDERS: { value: Gender; label: string }[] = [
  { value: "kvinna", label: "Kvinna" },
  { value: "man", label: "Man" },
  { value: "annat", label: "Annat / vill inte säga" },
];

/** The profile form, used in onboarding and in Settings. */
export function ProfileFields({
  profile,
  onChange,
  part = "all",
  autoFocus = false,
}: {
  profile: Profile;
  onChange: (p: Profile) => void;
  part?: "you" | "work" | "all";
  autoFocus?: boolean;
}) {
  const set = (k: keyof Profile) => (v: string) => onChange({ ...profile, [k]: v });
  return (
    <div className="space-y-4">
      {part !== "work" && (
        <>
          <Field label="Namn">
            <input
              autoFocus={autoFocus}
              value={profile.name}
              onChange={(e) => set("name")(e.target.value)}
              placeholder="Förnamn Efternamn"
              className={inputClass}
            />
          </Field>
          <Field label="Kön" hint="Så att AI:n skriver hon, han eller hen om dig.">
            <div className="flex flex-wrap gap-1.5">
              {GENDERS.map((g) => (
                <button
                  key={g.value}
                  type="button"
                  onClick={() => set("gender")(g.value)}
                  className={`h-8 rounded-full px-3.5 text-[13px] font-medium transition-colors ${
                    profile.gender === g.value ? "bg-accent text-on-accent" : "bg-sunken text-ink hover:bg-hover"
                  }`}
                >
                  {g.label}
                </button>
              ))}
            </div>
          </Field>
        </>
      )}
      {part !== "you" && (
        <>
          <Field label="Företag">
            <input
              autoFocus={autoFocus && part === "work"}
              value={profile.company}
              onChange={(e) => set("company")(e.target.value)}
              placeholder="Där du jobbar"
              className={inputClass}
            />
          </Field>
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

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1.5">
      <span className="block text-[12.5px] font-medium text-muted">{label}</span>
      {children}
      {hint && <span className="block text-[11.5px] text-faint">{hint}</span>}
    </label>
  );
}
