#import "../design.typ": *

#appendix(letter: "B", title: "Glossary")

Every term this book defined, gathered in one place and in alphabetical order —
both the worldbuilding ideas and the Inkhaven ones — so you can settle any word
without hunting for the chapter that first introduced it.

#gloss("Archetype")[A role a generated element plays rather than a named individual — a settlement's role archetype, or a flora, fauna, or keystone-animal archetype in the ecology. The world sketches the type; you write the particular.]

#gloss("Authority")[The discipline that the world proposes and the author decides. Nothing the world generates is written into your manuscript without you accepting it, and you may always override what it inferred. The author has the last word.]

#gloss("Axial tilt")[The angle between a planet's spin axis and the line straight up from its orbit — Earth's is about 23.4°. It is why there are seasons at all, and the single most consequential number in the sky.]

#gloss("Biome")[A large-scale kind of living landscape defined by its climate — the world recognises twelve, from ice_cap and tundra through the forests, grasslands, and deserts to tropical_rainforest and ocean. Biomes aggregate into climate zones and decide what can live and grow where.]

#gloss("Canon")[What is true in your world — the settled body of facts your prose must answer to. The compiled world is the source of it; the fact-checker measures your sentences against it.]

#gloss("Capital")[The largest, best-sited settlement of a polity, around which its cluster of cities is gathered. Because prime sites are claimed first, a capital is usually also among the oldest cities.]

#gloss("Carrying capacity")[How large a population a patch of land can support, given its climate, water, and fertility. It is why cities grow where they do — river mouths and fertile valleys carry more people than marginal ground.]

#gloss("Chronology")[The ordering of events along time — what came before what, and how long ago. Your world's chronology is inferred from where its cities sit and how large they grew, and dated on the world's own calendar.]

#gloss("Compile")[To run the world's layers, in order, turning the small world.hjson definition into a full world — plates, climate, rivers, settlements, and more. Compiling is pure and repeatable: the same definition always yields the same world.]

#gloss("Consistency")[The property that the pieces of a world hold together and never contradict one another. Good worldbuilding is measured less by how much you invent than by how well it stays consistent; the compiler and fact-checker exist to protect it.]

#gloss("Continent")[A large landmass raised by the geology layer where tectonic plates meet and part. Continents, their coastlines, and the seas between them all follow from the seed, not from a hand-drawn map.]

#gloss("Continuity")[Consistency held across time and across your manuscript — the same tower the same age in chapter three and chapter twelve, the same season the same date wherever it recurs. The shared root of calendar and climate is what keeps continuity from drifting.]

#gloss("Culture")[The way of life of a people — one per polity — with an ethos drawn from its capital's biome, a belief, a language profile, and a naming sample. Cultures give a world its peoples, not just its populations.]

#gloss("DEM")[A digital elevation model — a real heightmap image. If geology.dem is set, the geology layer builds its terrain from that image instead of from the seed, letting you ground a world on real or hand-made relief.]

#gloss("Demographics")[The last physical layer: where people settle and how many. Derived from climate and hydrology, it places settlements, assigns populations and role archetypes, and arranges them into a rank-size hierarchy.]

#gloss("Deterministic")[Producing the same result every time from the same inputs. The world compiler is fully deterministic and runs offline — a given definition and seed always grow exactly the same world, so your world is reproducible and shareable.]

#gloss("Ecology")[The living layer of the world — flora and fauna archetypes per biome, with a keystone animal for each land biome — read out by realworld ecology so a scene has plants and creatures consistent with its climate.]

#gloss("Emergence")[The way each layer arises as a consequence of the ones before it — the sun shaping the climate, the climate carving the rivers, the rivers deciding where cities stand. You set initial conditions; the world emerges from them rather than being placed by hand.]

#gloss("Epoch")[A named stretch of a world's history with its own character. The history command produces three, oldest to most recent: the Founding Age, the Age of Expansion, and the Present Age.]

#gloss("Ethos")[The character of a culture, drawn from its capital's biome — settled and water-minded for a river valley, harder for a desert's edge. The ethos is the seed of how a people think and behave.]

#gloss("Exception to physics")[A rule you deliberately declare in the magic ledger that overrides what the physical world would otherwise insist on — a road that folds distance, a season that does not behave. Once declared, the fact-checker honours it and stops warning about it.]

#gloss("Fact-check")[Checking your prose against the compiled world — travel time, climate, date coherence, and more — with realworld fact-check. Declared magic exceptions are suppressed, so a one-time setup does not produce endless false warnings.]

#gloss("Gazetteer")[A consolidated Markdown world reference — calendar, sky, regions, landmarks, waters, settlements, economy, and magic in one document — produced by realworld gazetteer for reading or sharing.]

#gloss("Insolation")[The amount of a star's light and heat falling on a patch of ground over time. It varies with latitude and season, and is the raw energy budget the climate layer spends to make temperature and rain.]

#gloss("Keystone species")[The one animal per land biome that anchors its ecology — the creature a scene in that biome would most naturally put on the page. The ecology layer names one for each land biome.]

#gloss("Layer")[One stage of the compiled world — astronomy, geology, climate, hydrology, or demographics — each computed in order and each a consequence of the ones before it. The layers are the physical world, grown one at a time.]

#gloss("Magic ledger")[The magic block of world.hjson: declared exceptions to physics, each a rule naming what it covers and whom it applies to. The fact-checker consults it so a deliberate departure from physics is honoured rather than flagged.]

#gloss("Materialize")[To write the compiled world down as readable chapters in the World system book, with compile --materialize (or history --materialize). Materialising records the world's proposal; it does not touch your manuscript.]

#gloss("Polity")[A nation — a cluster of settlements gathered around a capital, with a name, a population, and seeded relations (allied, rival, or neutral) with its neighbours. Polities are read out of the settled world by realworld polities.]

#gloss("Proposal")[Something the world offers for you to accept or reject — a settlement as a Place, a calendar, a history event. The world proposes; you decide; only what you accept reaches your book.]

#gloss("Rain shadow")[The dry region in the lee of a mountain range, where winds arrive having already spent their moisture climbing the windward side. It is why the far side of a range can be desert while the near side is green.]

#gloss("Rank-size hierarchy")[The orderly spread of settlement sizes — a few large cities, more towns, many villages — that the demographics layer arranges, mirroring how real settlement sizes distribute rather than clumping at one scale.]

#gloss("Scene context")[What the desk knows about the scene at your cursor — its place, season, and people — surfaced by realworld scene, the ambient footer chip, and the "This scene" header in the World overview. The world present while you write.]

#gloss("Seed")[A single number that fixes all the \"random\" choices a world involves — where coastlines fall, which valley grows the first city. The same seed always produces the same world, so your world is reproducible.]

#gloss("Settlement")[A place people live — classed as a city, town, or village — placed by the demographics layer where the land supports it, on the good sites the hydrology marked: river mouths, confluences, fertile valleys.]

#gloss("Solstice / Equinox")[The four turning points of the year. At an equinox the tilt favours neither hemisphere, so day and night are equal; at a solstice one hemisphere leans most toward the star (its summer) and the other away (its winter).]

#gloss("Synodic period")[The time a moon takes to return to the same phase as seen from the ground — new moon to new moon. It differs from the raw orbital period because the planet is itself moving, and it is the rhythm a lunar month keeps.]

#gloss("System book")[A dedicated project-wide book Inkhaven keeps beside your manuscript — Places, Characters, the Timeline, and the World book — readable, searchable, and citable from your prose.]

#gloss("Tectonic plate")[A section of the planet's crust whose movements the geology layer simulates. Where plates meet and part, the world raises mountains, opens seas, and shapes continents.]

#gloss("The World book")[The system book that holds your compiled world — every materialised layer written as readable chapters, opened read-only with Ctrl+B W. It is where the world you grew is written down.]

#gloss("Watershed")[The area of land that drains to a single river or lake — the catchment whose rainfall a river gathers. The hydrology layer traces watersheds as it runs water downhill across the terrain.]

#gloss("world.hjson")[The single file that defines a world: its name, seed, primary language, and the blocks — astronomy (required), geology, geography, hydrology, economy, magic. Everything the compiler grows begins here.]

#gloss("Worldbuilding")[The craft of inventing a setting — its geography, history, peoples, and rules — thoroughly and consistently enough that no reader ever catches it contradicting itself. Less about how much you invent than about how well the pieces hold together.]
