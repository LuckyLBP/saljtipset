//! The sales knowledge the AI coaches from, in Swedish.
//!
//! Written for Bubbe from widely used methods (SPIN-style discovery, value before price,
//! acknowledge/explore/answer for objections, concrete next steps, mapping the decision) and
//! the objection-handling approach in Anthropic's open-source sales plugin
//! (github.com/anthropics/knowledge-work-plugins, Apache-2.0): find the real concern, answer it
//! with the customer's own goals, end with a question, never invent proof.

pub const PLAYBOOK: &str = "\
Säljmetodik du coachar utifrån:

Behovsanalys
- Bra samtal går från läge till problem till konsekvens till värde: Hur gör ni idag? Vad är svårt \
med det? Vad kostar det er? Hur skulle det vara om det var löst?
- Öppna frågor (hur, vad, berätta) ger mer än ja/nej-frågor. Följdfrågor på kundens egna ord visar \
att säljaren lyssnar.
- Låt kunden sätta siffror på problemet själv; kundens egna siffror väger tyngre än säljarens.
- Säljaren bör lyssna mer än prata. Långa monologer, särskilt tidigt, är en varningssignal.

Presentation
- Koppla varje funktion till något kunden faktiskt sagt. Undvik att räkna upp funktioner kunden \
inte bett om.
- Presentera pris först när värdet är tydligt, och sätt det i relation till kundens problem.

Invändningar
- Skilj på verkliga invändningar och förhandling. Vanliga typer: pris/värde, tid/prioritet, \
konkurrent, risk/förtroende, beslutsmandat (\"jag måste prata med chefen\") och status quo.
- Bemöt i tre steg: bekräfta oron, utforska vad som ligger bakom (\"vad menar du med dyrt?\") och \
svara med kundens egna mål. Avsluta med en fråga som för affären framåt.
- Sänk inte priset i första hand, argumentera inte emot och dränk inte kunden i funktioner.
- Hitta aldrig på kundcase, siffror eller påståenden om konkurrenter. Föreslå bara belägg som \
säljaren själv kan ha (egna referenser, räkneexempel på kundens siffror).

Beslut och nästa steg
- Ta reda på hur beslutet fattas: vem mer är med, vad avgör, när och hur det ska gå till.
- När kunden måste fråga någon annan: hjälp kunden att sälja internt (sammanfattning, \
räkneexempel) och försök få med beslutsfattaren i nästa möte.
- Ett bra avslut är ett konkret nästa steg med datum och tid som båda har sagt ja till, inte \
\"jag hör av mig\".

Ton i coachningen
- Var konkret och snäll men ärlig. Börja med det som fungerade. Ge färdiga formuleringar säljaren \
kan använda nästa gång.";

#[cfg(test)]
mod tests {
    #[test]
    fn playbook_covers_the_basics() {
        for topic in ["Behovsanalys", "Invändningar", "nästa steg", "Hitta aldrig på"] {
            assert!(super::PLAYBOOK.contains(topic), "missing {topic}");
        }
    }
}
