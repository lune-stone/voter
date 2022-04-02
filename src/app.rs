use itertools::Itertools;
use rand::distributions::Distribution;
use rand::distributions::Uniform;
use rand::thread_rng;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Cursor;
use std::str::FromStr;
use std::string::ToString;
use strum::IntoEnumIterator;
use strum_macros::Display;
use strum_macros::EnumIter;
use strum_macros::EnumString;
use tallystick::plurality::DefaultPluralityTally;
use tallystick::schulze::SchulzeTally;
use tallystick::schulze::Variant;
use tallystick::util;
use wasm_bindgen::JsCast;
use yew::prelude::*;

type WeightedUnrankedVote = Vec<(String, u32)>;
type RankedVote = Vec<(String, u32)>;
type WeightedRankedVote = Vec<(RankedVote, u32)>;
type Ranking = Vec<(String, usize)>;

#[derive(Debug, Display, PartialEq, EnumString, EnumIter)]
#[strum(serialize_all = "title_case")]
enum Method {
    Plurality,
    SchulzeWinning,
    WeightedRandom,
}

fn parse_votes(raw: &String) -> anyhow::Result<WeightedRankedVote> {
    let votes = Cursor::new(raw);
    if let Ok(v) = util::read_votes(votes) {
        let mut out = vec![];
        for x in v {
            let xx = x.0.into_ranked();

            let set: HashSet<String> = xx.iter().map(|(c, _)| c.to_string()).collect();
            if set.len() < xx.len() {
                Err(anyhow::anyhow!(
                    "Failed to parse votes, canidate was used twice in a ranking."
                ))?
            }

            out.push((xx, x.1));
        }
        Ok(out)
    } else {
        Err(anyhow::anyhow!(
            "Failed to parse votes, check for stray '*', '>', or '=' characters."
        ))
    }
}

fn as_unranked_votes(votes: &WeightedRankedVote) -> anyhow::Result<WeightedUnrankedVote> {
    if votes
        .iter()
        .any(|x| x.0.iter().filter(|y| y.1 == 0).count() > 1)
    {
        Err(anyhow::anyhow!("Vote has more than one first choice."))
    } else {
        Ok(votes
            .iter()
            .filter(|x| !(x.0.is_empty()))
            .map(|x| {
                (
                    x.0.iter()
                        .filter(|y| y.1 == 0)
                        .map(|y| y.0.to_string())
                        .next()
                        .unwrap_or_default(),
                    x.1,
                )
            })
            .collect())
    }
}

fn consolidate_unranked_votes(votes: &WeightedUnrankedVote) -> WeightedUnrankedVote {
    let mut map = HashMap::new();
    for (canidate, weight) in votes {
        map.entry(canidate.to_string())
            .and_modify(|value| *value += *weight)
            .or_insert(*weight);
    }
    map.into_iter().collect_vec()
}

fn candidates_from_votes(votes: &WeightedRankedVote) -> Vec<String> {
    let candidates: HashSet<String> = votes
        .iter()
        .flat_map(|x| x.0.iter().map(|y| y.0.clone()))
        .collect();
    let candidates: Vec<String> = candidates.into_iter().collect();
    candidates
}

fn plurality(votes: &WeightedUnrankedVote, candidates: &Vec<String>) -> Ranking {
    let mut tally = DefaultPluralityTally::new(candidates.len());

    for (vote, weight) in votes {
        tally.add_weighted(vote.clone(), (*weight) as u64);
    }

    tally
        .winners()
        .iter()
        .map(|i| (i.candidate.to_string(), i.rank))
        .collect()
}

fn schulze(votes: &WeightedRankedVote, candidates: Vec<String>) -> anyhow::Result<Ranking> {
    if candidates.len() == 1 {
        // avoid minor bug in tallystick where single canidate doesn't produce a winner
        return Ok(vec![(candidates[0].clone(), 0)]);
    }

    let mut tally: SchulzeTally<String, u32> =
        SchulzeTally::with_candidates(candidates.len(), Variant::Winning, candidates);

    for (vote, weight) in votes {
        let r = tally.ranked_add_weighted(vote, *weight);
        if r.is_err() {
            return Err(anyhow::anyhow!(
                "Invalid vote was used. Check that vote order does not list canidate twice."
            ));
        }
    }

    Ok(tally
        .winners()
        .iter()
        .map(|i| (i.candidate.to_string(), i.rank))
        .collect())
}

fn weighted_random(votes: &WeightedUnrankedVote) -> Ranking {
    let mut rng = thread_rng();

    let mut votes = consolidate_unranked_votes(votes);
    let mut winners: Vec<String> = vec![];
    while !votes.is_empty() {
        let sum: u32 = votes.iter().map(|x| x.1).sum();
        let mut roll = Uniform::from(1..sum + 1).sample(&mut rng);

        let mut found_index: usize = 0;
        loop {
            let weight = votes[found_index].1;
            if roll <= weight {
                break;
            }
            roll -= weight;
            found_index += 1;
        }

        winners.push(votes.swap_remove(found_index).0);
    }

    winners
        .iter()
        .enumerate()
        .map(|(x, y)| (y.to_string(), x))
        .collect()
}

fn vote(votes_raw: &str, method: &str) -> anyhow::Result<Vec<(String, usize)>> {
    let votes = parse_votes(&votes_raw.to_string())?;
    let candidates = candidates_from_votes(&votes);
    let method = Method::from_str(method)?;

    let winnings = match method {
        Method::SchulzeWinning => schulze(&votes, candidates)?,
        Method::Plurality => plurality(&as_unranked_votes(&votes)?, &candidates),
        Method::WeightedRandom => weighted_random(&as_unranked_votes(&votes)?),
    };

    Ok(winnings)
}

#[function_component(App)]
pub fn app() -> Html {
    let instructions = r#"
    === Voter ===

    Voter is a simple tool to rank candidates calculated by various popular and useful algorithms.

    Collecting votes is outside the scope of this tool. If you need this feature consider using another tool like https://www.condorcet.vote/ instead.
    
    For more info or to provide feedback please visit https://github.com/lune-stone/voter

    === Instructions ===

    For a simple unranked poll (ex: what is your favorite fruit?) you can submit votes one per line like so:

    Strawberry
    Apple
    Banana
    Apple

    Candidate names are inferred from the ballets. If a candidate receives zero votes, they will not be included in the results.

    Any characters except '*', '>', and '=' are considered valid and part of the name of the candidate.

    You also have the choice to use '*' to add weight to votes like so:

    Strawberry
    Apple * 2
    Banana

    (An unweighted vote is the same as weight 1).

    To represent a ranked vote use '>' or '=' between the candidates like so:
    
    Strawberry > Apple > Banana
    Strawberry > Banana = Apple * 5
    Banana > Apple > Strawberry * 3
    Apple 

    The '>' indicates a preference for the left candidate, '=' indicates no preference between the candidates on the left and right.

    You can omit candidates on a ranked vote to express that the omitted candidates have the lowest rank. In other words both of these lines are functionally the same:

    Strawberry > Banana = Apple
    Strawberry 


    === Explanation of Algorithms ===

    --- Plurality --- 

    An unranked voting algorithm (each vote picks one candidate). The winners are picked based on who has the most votes.

    If ranked votes are submitted, only the first choice is used.
    
    https://en.wikipedia.org/wiki/Plurality_(voting)

    --- Weighted Random --- 

    Also known as a lottery.
    
    An unranked voting algorithm (each vote picks one candidate). The winners are picked at random picking from the pool of votes.
    
    If ranked votes are submitted, only the first choice is used.

    https://en.wikipedia.org/wiki/Random_ballot

    --- Schulze (Winning Variant) --- 

    A ranked voting algorithm (each vote orders the candidates). The winners are picked using a complicated process that ranks each candidate based on how well they polled overall.

    https://en.wikipedia.org/wiki/Schulze_method
    "#;

    let instructions = instructions.trim().lines().map(|x| x.trim()).join("\n");

    let raw_votes = use_state(|| instructions.to_string());
    let alg = use_state(|| Method::Plurality.to_string());
    let winners: UseStateHandle<Vec<(String, usize)>> = use_state(Vec::new);
    let error = use_state(|| "".to_string());

    let onclick = {
        let raw_votes = raw_votes.clone();
        let method = alg.clone();
        let winners = winners.clone();
        let error = error.clone();
        Callback::from(move |_| match vote(&raw_votes, &method) {
            Ok(w) => {
                winners.set(w);
                error.set("".to_string());
            }
            Err(e) => {
                winners.set(vec![]);
                error.set(e.to_string());
            }
        })
    };

    let oninput = {
        let raw_votes = raw_votes.clone();
        let winners = winners.clone();
        let error = error.clone();
        Callback::from(move |e: InputEvent| {
            winners.set(vec![]);
            error.set("".to_string());
            if let Some(data) = e.target().and_then(|event_target: web_sys::EventTarget| {
                event_target.dyn_into::<web_sys::HtmlTextAreaElement>().ok()
            }) {
                raw_votes.set(data.value());
            }
        })
    };

    let onchange = {
        let alg = alg.clone();
        let winners = winners.clone();
        let error = error.clone();
        Callback::from(move |e: Event| {
            winners.set(vec![]);
            error.set("".to_string());
            if let Some(target) = e.target().and_then(|event_target: web_sys::EventTarget| {
                event_target.dyn_into::<web_sys::HtmlSelectElement>().ok()
            }) {
                alg.set(target.value());
            }
        })
    };

    html! {
        <div>
            <h1> { "Voter" } </h1>
            <div>
                <p>{"Votes: "}</p>
                <textarea {oninput} value={(*raw_votes).to_string()}></textarea>
            </div>
            <div>
                <p>{"Method: "}
                    <select {onchange}>
                        { for Method::iter()
                                .map(|x| html!{ <option selected={ (*alg) == x.to_string() }> {x.to_string()} </option> })
                        }
                    </select>
                </p>
                <button {onclick}>{ "Calculate" }</button>
            </div>
            if !(*error).is_empty() {
                <div class="error">
                    <p>{"Error: "} {(*error).to_string()}</p>
                </div>
            }
            if !(*winners).is_empty() {
                <div>
                    <table>
                    <thead>
                    <th>{"candidate"}</th>
                    <th>{"rank"}</th>
                    </thead>
                    <tbody>
                    { for (*winners).iter()
                        .cloned()
                            .map(|(canidate, rank)|
                                html!{
                                    <tr>
                                        <td> {canidate} </td>
                                        <td> {rank + 1} </td>
                                    </tr>
                                }) }
                        </tbody>
                    </table>
                </div>
            }

        </div>
    }
}
