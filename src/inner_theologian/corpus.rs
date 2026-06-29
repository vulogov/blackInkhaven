//! INNER-THEOLOGIAN-1 (IT-P4) — the slow-track question corpus.
//!
//! Audit correction: the RFC asked for "the full corpus in all five languages as
//! localised constants." That is **not** how this codebase localises an LLM
//! feature. `inner_socrates::slow::build_slow_prompt` keeps English templates and
//! adds a directive — "the paragraph is in {language}; write each question in
//! {language} and its question_en in English" — letting the model render in the
//! project language. We follow that exactly (IT-P5). So this corpus is English
//! templates; tradition-specific terms are glossed inline so the model (and the
//! author) need no prior knowledge of the tradition.
//!
//! Each category carries a handful of question templates. The prompt builder
//! (IT-P5) selects a category and the most illuminating lenses for the passage,
//! injects the relevant templates, and the persona always names which lens raises
//! which question.

use super::QuestionCategory;

/// The English question templates for a category, in RFC §9.2 order.
pub(crate) fn questions_for(cat: QuestionCategory) -> &'static [&'static str] {
    match cat {
        QuestionCategory::MoralWeight => MORAL_WEIGHT,
        QuestionCategory::Theodicy => THEODICY,
        QuestionCategory::Redemption => REDEMPTION,
        QuestionCategory::Sacred => SACRED,
        QuestionCategory::Duty => DUTY,
        QuestionCategory::ImplicitTheology => IMPLICIT_THEOLOGY,
    }
}

/// The single auto-fire question (paragraph idle): always the cost-of-harm
/// question from Category 1 (RFC §7.1).
pub(crate) fn auto_fire_question() -> &'static str {
    MORAL_WEIGHT[0]
}

const MORAL_WEIGHT: &[&str] = &[
    "When the events of this passage cause harm, what is the depicted cost — to the one who caused it, \
     to the one who received it, and to the world of the story? Is that cost proportionate? From a \
     consequentialist view (secular philosophy): does the narrative reckon with outcomes? From a \
     Buddhist view: does the harm arise from craving, and is that causal chain visible? From a Jewish \
     view: is repair (teshuvah — turning back toward what was broken) addressed, not just blame?",
    "Who suffers in this passage, and whose suffering is made visible to the reader — and whose is \
     not? The choice of whose suffering to depict is itself a moral act. From a care-ethics view \
     (secular philosophy): are the inner experiences of vulnerable, dependent characters present in \
     the prose, or background to the choices of more powerful ones?",
    "This passage depicts violence. Does the narrative treat it as having moral weight — leaving a \
     mark on characters and world — or as spectacle? From a Gnostic view: does the violence feel like \
     a feature of a broken world, or of how stories work? From an LDS view (the body as a sacred \
     gift, not a temporary vessel): what does its treatment of bodily harm say about how it values \
     the physical?",
    "When harm occurs, how do witnesses respond? Silence, indifference, and complicity are moral \
     positions. From an Islamic view (amr bil ma'ruf wa nahy 'an al-munkar — enjoining good and \
     forbidding evil): does any witness face the obligation to act, and does the narrative treat \
     their response as morally significant?",
];

const THEODICY: &[&str] = &[
    "There is suffering here that does not follow from the character's choices. The narrative offers \
     no / an implicit / an explicit reason for it — and that choice is a theological position. \
     Augustinian Christian: suffering is consequence of the Fall, not randomness. Jewish (Job): \
     suffering may be real and undeserved, and the response may be argument, not acceptance. \
     Buddhist: suffering (dukkha) arises from craving; the path is through it, not a reason for it. \
     Gnostic: suffering is evidence of the demiurge's flawed creation. Which does your narrative most \
     resemble, and is that intentional?",
    "This suffering does not advance the story, illuminate character, or produce consequence. Is that \
     intended? Meaningless suffering is itself a statement — that the universe is indifferent to \
     moral effort (Camus made it the basis of an ethics). Legitimate, but it should be chosen, not an \
     oversight.",
    "From the Jewish tradition: Job is righteous, suffers immensely, and argues with God — and the \
     tradition treats that argument as holy, perhaps more than silent acceptance. Does any character \
     argue with their circumstances the way Job does, or do they endure, accept, or ignore? What does \
     the narrative believe about the legitimacy of protest against suffering?",
    "From a Buddhist view: anicca (impermanence) is fundamental, and suffering often comes from \
     treating impermanent things as permanent. Does any character suffer specifically because they \
     cannot accept change — and does the narrative meet that with compassion, judgment, or \
     indifference?",
];

const REDEMPTION: &[&str] = &[
    "Does any character undergo genuine moral transformation — not changed circumstances but changed \
     values and orientation? What does the narrative say is required for it? Catholic/Orthodox: grace, \
     something given, not earned. Protestant: faith and surrender. Buddhist: cessation of craving \
     through practice. Confucian: sustained cultivation through right relationship. LDS: eternal \
     progression — becoming different in kind, not just degree. Which does your vision resemble, and \
     is there a depicted, proportionate cost?",
    "A character commits a serious wrong. What path to redemption does the narrative offer — \
     forgiveness (by whom?), restitution (to whom?), suffering (how much?), grace (from what \
     source?)? From the Jewish teshuvah: genuine repentance is recognition, remorse, repair, and \
     resolve not to repeat — does the path include all four? From the LDS infinite atonement: none is \
     beyond reach except those who fully knew the truth and wholly rejected it — does your vision of \
     irredeemability match that?",
    "Is any character treated as irredeemable? What does the narrative say about that — deserved, \
     tragic, or simply realistic? From a Gnostic view: the purely hylic (material, no divine spark) \
     cannot receive gnosis — irredeemability as ontology, not morality. From most traditions it is \
     denied or treated as tragedy. What does your narrative believe?",
    "Flannery O'Connor: 'There is a moment in every great story in which the presence of grace can be \
     felt as it waits to be accepted or rejected, even though the reader may not recognise it.' Is \
     there such a moment here — something offered and a choice made, named as grace or not? Is the \
     offer visible in the prose, or only in your intention?",
];

const SACRED: &[&str] = &[
    "Is there anything the characters treat as sacred — irreducibly valuable, not to be traded off or \
     instrumentalised? It need not be supernatural: a relationship, a principle, a promise can be \
     sacred in a fully secular story, but something must function so for the story to have moral \
     stakes. From an LDS view (the body, the family, the individual spirit as inherently sacred): \
     what has that quality of non-negotiable worth here?",
    "Rudolf Otto described the sacred as mysterium tremendum et fascinans — wholly other, terrifying \
     and fascinating. Not exclusively religious: the uncanny, the sublime, the confrontation with \
     what exceeds comprehension all carry numinous weight. Does this work contain such a moment — and \
     does the prose hold it with weight, or resolve it too quickly into ordinary narrative logic?",
    "Does the world of this novel have a moral structure — do events ultimately mean something, does \
     suffering lead somewhere, is goodness vindicated and evil accountable — or is its universe \
     indifferent? Both are legitimate (Camus and Beckett chose indifference; most religious \
     traditions a moral order). Which does your novel believe, and does the prose enact it or \
     contradict it?",
    "From a Gnostic view: many narratives (especially fantasy, SF, dystopia) hide a structure where \
     an apparent reality conceals a truer one — awakened characters see through the veil, the \
     apparent world is corrupt, the true one obscured. Does your narrative have this structure? If so, \
     what is your demiurge (the false reality characters are kept in) and your pleroma (what awakening \
     reveals)?",
];

const DUTY: &[&str] = &[
    "Which characters here have duties to each other — from role (parent, ruler, healer), \
     relationship (covenant, friendship, debt), or promise? Which are honoured, which violated? From \
     a Confucian view: the five relationships (wu lun) each carry mutual obligations — does the \
     narrative treat them as real and weighty? From a Jewish view: mitzvot are owed, not chosen — are \
     relational obligations something characters owe, or merely choose?",
    "From an Islamic view of khalifa (stewardship): power, knowledge, wealth, and authority are \
     trusts held on behalf of something larger, not possessions. Which characters hold power? Do they \
     act as trustees — accountable for what is not ultimately theirs — or as proprietors? Does the \
     narrative distinguish these?",
    "From a care-ethics view (Gilligan, Noddings): moral life is the web of particular relationships \
     and dependencies, and the most serious obligations are often the least visible. Who here is in a \
     relationship of care or dependency, vulnerable to another's choices? Does the narrative attend \
     to their inner lives, or are they background to the powerful?",
    "From a Kantian view (secular philosophy): the categorical imperative forbids treating persons \
     merely as means, however good the ends. Does any character use another — or allow them to be \
     used — as a means? Does the narrative take the moral cost seriously, or does ends-justify-means \
     logic go unexamined?",
];

const IMPLICIT_THEOLOGY: &[&str] = &[
    "Every novel has an implicit theology — assumptions about whether moral effort matters, whether \
     the universe has a moral structure, whether suffering has meaning, what humans are capable of — \
     enacted in what the narrative rewards, punishes, and treats as inevitable. Reading this work \
     whole: what does it appear to believe? I will offer a reading; tell me where it is wrong.",
    "Is there a tension between the values the narrative espouses and those it enacts? A story can \
     claim to be about justice while depicting justice as impossible, or value dignity while treating \
     some suffering as background. From a Confucian view of zhengming (rectification of names): does \
     this work call things what they are, or name values it does not enact?",
    "Of the eleven tradition lenses, which does this work's moral cosmology most resemble in \
     underlying structure (not explicit content) — about whether humans can change, whether goodness \
     is possible, whether suffering means anything, whether the universe is morally ordered? Which \
     tradition would feel most at home in this narrative's moral universe, and which most estranged — \
     and what does that tell you about the work you are making?",
    "From an LDS view: humans are eternal intelligences with unbounded potential; the limits of \
     transformation are not fixed. Does this narrative treat human potential as bounded (people \
     improve but stay human in kind) or unbounded (characters can become different in kind)? What \
     does it believe transformation can ultimately achieve — and is that enacted in its characters' \
     arcs?",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_has_questions_and_glosses_terms() {
        for n in 1..=6u8 {
            let cat = QuestionCategory::from_number(n).unwrap();
            let qs = questions_for(cat);
            assert!(qs.len() >= 3, "category {n} too few questions");
            for q in qs {
                assert!(q.len() > 80, "question too short to be a real template");
            }
        }
        // The corpus names its source traditions inline (a spot-check that the
        // lens framings survive).
        assert!(questions_for(QuestionCategory::Theodicy).iter().any(|q| q.contains("Job")));
        assert!(questions_for(QuestionCategory::Redemption).iter().any(|q| q.contains("teshuvah")));
        assert!(questions_for(QuestionCategory::Sacred).iter().any(|q| q.contains("pleroma")));
    }

    #[test]
    fn auto_fire_is_category_one_cost_of_harm() {
        assert_eq!(auto_fire_question(), MORAL_WEIGHT[0]);
        assert!(auto_fire_question().contains("cost"));
    }
}
