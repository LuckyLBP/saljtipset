import type { AppInfo } from "../api";
import { Mark, TAGLINE, Wordmark } from "../brand";
import ModelPicker from "./ModelPicker";

export default function Setup({ info, onDone }: { info: AppInfo; onDone: () => void }) {
  return (
    <div className="flex h-full flex-col overflow-auto">
      <div className="h-[52px] shrink-0" data-tauri-drag-region />
      <div className="flex flex-1 items-center justify-center px-10 pb-12">
        <div className="w-full max-w-3xl space-y-8">
          <div className="flex flex-col items-center gap-3 text-center">
            <Mark size={72} />
            <Wordmark size={30} />
            <p className="text-[13px] text-muted">{TAGLINE}</p>
          </div>
          <div className="content space-y-5 rounded-[28px] p-7 shadow-[0_0_0_0.5px_var(--color-line),0_20px_60px_rgba(0,0,0,0.08)]">
            <div className="space-y-1.5">
              <h1 className="font-display text-[20px] font-semibold tracking-[-0.015em]">Ladda ner talmodellen</h1>
              <p className="max-w-2xl text-[13px] leading-relaxed text-muted">
                Säljtipset skriver ut dina samtal med KB-Whisper, en svensk talmodell från Kungliga biblioteket. Modellen
                körs på din dator, så ljudet lämnar aldrig datorn. Den behöver bara laddas ner en gång.
              </p>
            </div>
            <ModelPicker info={info} onChanged={onDone} />
          </div>
        </div>
      </div>
    </div>
  );
}
