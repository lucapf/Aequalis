# Clothing and shoe sizes — specification

Status: **agreed.** Every decision is settled (§10); implementation follows the
phases (§8).

## 1. Scope and non-goals

Aequalis converts clothing and shoe sizes between countries' systems, and from a
body measure to a size. Examples:

- `42 eu men shoes us` → `US 8.5 (Nike), 9 (Clarks)`
- `IT 44 dress US` → `US 8`
- `27 cm men foot` → `EU 44 (Nike), 43 (Clarks)`

It answers the way it answers units and currencies: as you type, asking when a
request could mean more than one thing.

**Sizes only when asked for.** A request is a size request only when it says so,
with an item word (`shoes`, `dress`, `clothes`…) or a body word (`foot`).
Everything else reads as before, currencies first: `42 eu` is 42 euros. When
such a currency request could also be a shoe size, the answer adds the sizes as
a second result, in a list, never in place of the currency (§3.2).

Non-goals:

- **Two shoe charts, no more.** Nike's and Clarks's are named in the result
  where they differ (§6.1). There are no other brand charts, and every answer
  says brands vary (§4.3).
- **No fit advice:** no "size up for boots", nothing about widths beyond what
  the charts give.
- **No vanity sizing history.** The charts are today's usual retail charts.

Sizes are **not units**. There is no factor between EU 42 and US 9. Each item
has a **chart**: rows of equivalent sizes across systems, each row standing for
a body measure. A conversion finds the row and reads the target column. A size
between two rows takes the nearest (§4.4).

## 2. Vocabulary

### 2.1 Systems

| Code       | Means                              | Used for                         |
|------------|------------------------------------|----------------------------------|
| `eu`       | Europe (the continental system)    | shoes, clothing, bras, hats      |
| `uk`       | United Kingdom                     | all                              |
| `us`       | United States                      | all                              |
| `it`       | Italy                              | women's and men's clothing, bras |
| `fr`       | France (clothing: also ES, BE, PT) | women's clothing, bras           |
| `de`       | Germany (also AT, CH, NL, Nordics) | women's clothing                 |
| `jp`       | Japan                              | shoes (cm), women's clothing     |
| `cn`       | China                              | shoes, clothing                  |
| `int`      | International letters (XS–XXXL)    | tops, shirts, hats               |
| `cm`, `in` | The size is a measure              | foot, collar, head, height       |

- **Shoes** have no separate IT, FR or DE system. Those countries use `eu`, so
  `42 it shoe` is read as EU 42, with a note.
- **Clothing:** `eu` alone means the **DE** column, the EN 13402 body-based
  numbering, where it differs from FR or IT. A note says so.

### 2.2 Size labels

| Label              | Example                                              | Items              |
|--------------------|------------------------------------------------------|--------------------|
| number, half sizes | `42`, `9.5`, `9½`, `42⅔` (Adidas's thirds)           | shoes, clothing    |
| letters            | `XS`, `S`, `M`, `L`, `XL`, `XXL`/`2XL`, `XXXL`/`3XL` | tops, shirts, hats |
| band + cup         | `75B`, `34C`, `34DD`                                 | bras               |
| age                | `2T`, `4T`, `6-7`, `6 years`, `18M`                  | kids               |
| height             | `128`, `128 cm`                                      | kids               |

### 2.3 Item words (10 languages, accents optional, singular and plural)

| Item                    | en                   | es                | pt            |
|-------------------------|----------------------|-------------------|---------------|
| clothes (any clothing)  | clothes, clothing    | ropa              | roupa(s)      |
| shoe                    | shoe(s)              | zapato(s)         | sapato(s)     |
| dress (women's chart)   | dress(es)            | vestido(s)        | vestido(s)    |
| top / T-shirt (letters) | top, t-shirt, tee    | camiseta          | camiseta      |
| trousers                | trousers, pants      | pantalón          | calça         |
| shirt (collar)          | shirt                | camisa            | camisa        |
| suit / jacket           | suit, jacket, blazer | traje, chaqueta   | terno, casaco |
| bra                     | bra                  | sujetador, sostén | sutiã         |
| hat                     | hat, cap             | sombrero, gorra   | chapéu, boné  |

| Item                    | fr             | de                | id           |
|-------------------------|----------------|-------------------|--------------|
| clothes (any clothing)  | vêtements      | Kleidung          | pakaian      |
| shoe                    | chaussure(s)   | Schuh(e)          | sepatu       |
| dress (women's chart)   | robe(s)        | Kleid(er)         | gaun         |
| top / T-shirt (letters) | t-shirt, haut  | T-Shirt, Oberteil | kaos, atasan |
| trousers                | pantalon       | Hose              | celana       |
| shirt (collar)          | chemise        | Hemd              | kemeja       |
| suit / jacket           | costume, veste | Anzug, Sakko      | jas          |
| bra                     | soutien-gorge  | BH                | bra, beha    |
| hat                     | chapeau        | Hut, Mütze        | topi         |

| Item                    | tr           | vi              | it            |
|-------------------------|--------------|-----------------|---------------|
| clothes (any clothing)  | giysi        | quần áo         | abbigliamento |
| shoe                    | ayakkabı     | giày            | scarpa/scarpe |
| dress (women's chart)   | elbise       | váy             | vestito/abito |
| top / T-shirt (letters) | tişört       | áo thun         | maglietta     |
| trousers                | pantolon     | quần            | pantaloni     |
| shirt (collar)          | gömlek       | áo sơ mi        | camicia       |
| suit / jacket           | takım, ceket | com lê, áo vest | abito, giacca |
| bra                     | sütyen       | áo ngực         | reggiseno     |
| hat                     | şapka        | mũ              | cappello      |

| Item                    | pl                  |
|-------------------------|---------------------|
| clothes (any clothing)  | ubrania, odzież     |
| shoe                    | but/buty            |
| dress (women's chart)   | sukienka            |
| top / T-shirt (letters) | koszulka            |
| trousers                | spodnie             |
| shirt (collar)          | koszula             |
| suit / jacket           | garnitur, marynarka |
| bra                     | biustonosz, stanik  |
| hat                     | czapka, kapelusz    |

**Body words** (for `27 cm foot`):

| Measure       | en           | es               | pt                 | fr              |
|---------------|--------------|------------------|--------------------|-----------------|
| foot          | foot, feet   | pie              | pé                 | pied            |
| collar/neck   | collar, neck | cuello           | colarinho, pescoço | col, cou        |
| head          | head         | cabeza           | cabeça             | tête            |
| height (kids) | height, tall | altura, estatura | altura             | taille, hauteur |

| Measure       | de                 | id           | tr          | vi        |
|---------------|--------------------|--------------|-------------|-----------|
| foot          | Fuß                | kaki         | ayak        | chân      |
| collar/neck   | Kragen, Hals       | kerah, leher | yaka, boyun | cổ        |
| head          | Kopf               | kepala       | kafa, baş   | đầu       |
| height (kids) | Größe, Körpergröße | tinggi       | boy         | chiều cao |

| Measure       | it               | pl                |
|---------------|------------------|-------------------|
| foot          | piede            | stopa             |
| collar/neck   | collo            | kołnierzyk, szyja |
| head          | testa            | głowa             |
| height (kids) | altezza, statura | wzrost            |

### 2.4 Person words (skip the men / women / kids question)

|       | en                            | es                         | pt                       |
|-------|-------------------------------|----------------------------|--------------------------|
| men   | men, man, mens, male          | hombre(s), caballero       | homem, homens, masculino |
| women | women, woman, ladies, female  | mujer(es), dama            | mulher(es), feminino     |
| kids  | kids, child(ren), boys, girls | niño(s), niña(s), infantil | criança(s), infantil     |

|       | fr        | de                   | id                |
|-------|-----------|----------------------|-------------------|
| men   | homme(s)  | Herren, Mann, Männer | pria, laki-laki   |
| women | femme(s)  | Damen, Frau, Frauen  | wanita, perempuan |
| kids  | enfant(s) | Kinder, Kind         | anak              |

|       | tr    | vi     | it            |
|-------|-------|--------|---------------|
| men   | erkek | nam    | uomo, uomini  |
| women | kadın | nữ     | donna, donne  |
| kids  | çocuk | trẻ em | bambino/a/i/e |

|       | pl                |
|-------|-------------------|
| men   | męskie, mężczyzna |
| women | damskie, kobieta  |
| kids  | dzieci, dziecięce |

- **Accents are optional**, as with place names: `ayakkabi`, `giay`, `fuss`,
  `kadin`,

- **Brands are not words.** `42 eu nike shoes us` is not a size request: the
  chart names appear only in answers (§6.1).

- **Words that already mean something** (decided 2026-10-22) count only beside
  another size word: `kid` and `top` (the Kiribati dollar and the Tongan
  paʻanga), `foot` and `feet` (the unit). `42 kid us` stays a currency and
  `27 cm foot` a length; `42 eu kid shoes us` and `27 cm men foot` are sizes.

- **Toddler and youth** (kids' shoes, decided 2026-10-22): toddler, toddlers,
  baby, infant, bebé, bebê, bébé, Kleinkind, balita, bebek, em bé, primi passi,
  maluch, niemowlę; youth, junior, juvenil, júnior, ado, Jugend, remaja, genç,
  thiếu niên, ragazzo, ragazzi, młodzież.

- **Possible clashes** are settled by the grammar (§3.4):
  - `but` (Polish shoe; English "but").
  - `pie` (Spanish foot).
  - `de` (DE; Spanish/Portuguese "of").
  - `it` (IT; English "it").
  - `in` (inches; "in" as `to`).
  - `boy` (Turkish "height"; English "boy" is a kids word).

## 3. Request grammar

### 3.1 The four forms (any order of the parts, as you'd write them)

1. `42 eu shoe us` — the size, its system, the item, the target system.
2. `shoe 42 eu` — the item first, no target: the region's system (§4.2).
3. `IT 44 dress US` — the system before the number, as on labels.
4. `27 cm foot` — a body measure, then the measure word; a target system is
   optional (`27 cm foot us`).

Optional connectors between the size and the target, as with units: `to`, `in`,
`->`, `→`, and their equivalents already accepted.

Person words go anywhere: `42 eu women shoe us`, `scarpe donna 38`.

A person word with no item word means clothing: `12 US woman EU` (§3.2).

### 3.2 What makes a request a size request

A request is a size request only when the user asks for one explicitly: **an
item word or a body-measure word** (§2.3), **and a size label** (§2.2). Anything
else is read as it is today, and **currency is the default**:

- `42 eu` stays 42 euros (`now the currency to convert to`), and `42 eu us`
  stays EUR → USD.
- When such a currency request could also be a shoe size, the answer is a
  **list**: the currency first, then a **shoes line** (§4.1).
- `42 in` stays 42 inches (a length).
- `cn` is not a currency code, so it never clashes.
- A **person word** alone counts too, for **clothing**:
  - `12 US woman EU` is the women's chart (dresses, tops and trousers share one,
    §6.2): `EU 42`.
  - `40 US man EU` asks which (§5): suits, collar shirts and letters are three
    charts.
  - Shoes always need the shoe word: `42 eu women us` is women's clothing, not
    shoes.

### 3.3 Partial requests (as you type)

| Typed        | Hint                                                                   |
|--------------|------------------------------------------------------------------------|
| `42 eu`      | the currency's answer or hint, as today, and the shoes line (§4.1)     |
| `shoe`       | `now the size: 42 eu, 9 us…`                                           |
| `shoe 42`    | `which system: eu, uk, us…` (or the region's, if 42 exists only there) |
| `42 eu shoe` | the table of every person and system at once (§4.1)                    |

### 3.4 Clashes

Without an item, body or person word nothing below applies: every word keeps its
usual meaning.

- **`in`** after a number with **no item word** is inches. After an item word
  it's a connector: `shoe 42 eu in us`. When it follows a body measure it's
  inches: `11 in foot`.
- **`it`, `de`** count as systems only next to a size (`it 44`, `44 it`) in a
  size request.
- **`but`, `pie`, `boy`** count as item/body words only in a size request (a
  size label present).

## 4. Answers

### 4.1 Shape

- **Result:** the target size with its system, `US 8.5` (`34C`, `M`).
  - **Shoes, where Nike's and Clarks's charts differ:** each value, named:
    `US 8.5 (Nike), 9 (Clarks)` (§6.1). Where they agree, the size alone:
    `US 8`.
- **Details**, one line, the other systems:
  `UK 7.5 · EU 42 · JP 26.5 cm · CN 42`. For shoes, these are Nike's.
- **Notes:**
  - which chart (`men's shoes`; for shoes split by brand,
    `men's shoes (details: Nike)`);
  - the "between" note (§4.4);
  - the brand note (§4.3).
- `--json`: `result`, `notes`; the details line is the first note.
- **Copy:**
  - the value: the first chart's (`8.5`);
  - the expression (`EU 42 = US 8.5 (Nike), 9 (Clarks)`), as with units.

**Shoes, not all specified** (decided 2026-10-22). Shoes never ask
`whose sizes?`: what is missing is shown.

- **No person** (`42 eu shoes us`): each person whose chart has the size, and
  only those, with Nike's value (Clarks's, named, when only it has the size):
  `US 8.5 men, US 10 women`; `30 eu shoes us` → `US 12.5C kids`;
  `50 eu shoes us` → `US 15.5 men (Clarks)`. Both charts once the person is
  typed.
- **No target** (`42 eu men shoes`): the region's system first, as the result,
  and every other system in the details (§4.2): `[us]` → `US 8.5 (Nike),
  9 (Clarks)`, details `UK 7.5 · EU 42 · JP 26.5 cm · CN 42`.
- **Neither** (`42 eu shoes`): a small table, a row per person who has the
  size, a column per system (US, UK, EU, JP, CN, the typed one included),
  Nike's values, the numbers lined up on the decimal point:

  ```text
  $ aequalis 42 eu shoes
           US     UK    EU     JP     CN
  men      8.5   7.5    42   26.5     42
  women   10     7.5    42   27       42
  ```

- **`--json` and Copy** (decided 2026-10-22):
  - no person: `result` is the first person's (`US 8.5 men`), `extra` the
    others, `"extra":[{"type":"shoes","results":["US 10 women"]}]`; Copy
    `8.5`;
  - the table: `result` is the table as text, `extra` one item per person,
    `{"type":"shoes men","results":["US 8.5","UK 7.5","EU 42","JP 26.5",
    "CN 42"]}`; Copy the table as text.
- **Shown below the request**: an answer of several lines (the table, the
  list beside a currency) takes the rows it needs under the request, in the
  TUI (live and in the history) and on Android (the result area grows, the
  table in a monospace font).

**Beside a currency** (decided 2026-10-22). A currency request whose amount and
codes could also be a shoe size (`42 eu us`) answers with a list, everywhere,
the CLI's stdout too (the rate is today's):

```text
$ aequalis 42 eu us
- 42 EUR = 47.15 USD
- shoes: 42 eu = US 8.5 men, US 10 women
```

- **When:** the amount is within a shoe chart's range (nearest rows, §4.4), and
  the codes are size systems that are also currency places: `eu`, `uk`, `us`,
  on either side (`42 eu us`, `9 us uk`). It is always shown, no `-v` needed.
  `it`, `fr`, `de`, `jp` and `cn` are no currency codes (`42 eu fr` is an
  error, as today), so they never bring the line (decided 2026-10-22).
- **Who:** each person whose chart has the size, and only those: men, women,
  kids. `47 eu us` → `US 12.5 men`; `36 eu us` → `US 5.5 women, US 4Y kids`;
  `30 eu us` → `US 12.5C kids`.
- **Which chart:** Nike's. A size only Clarks's chart has is Clarks's, named:
  `50 eu us` → `US 15.5 men (Clarks)`.
- **No notes:** the line is a summary. The notes (brand, between, Italian…) come
  with the size request itself (`42 eu men shoes us`).
- **No target** (`42 eu`): whatever the currency answers (the local currency, or
  the hint where that is the euro), and the line in the region's system (§4.2):
  `[us]` → `42 EUR = … USD` and `US 8.5 men, US 10 women`; in Dublin the hint
  and `UK 7.5 men, UK 7.5 women`. When the region's system is the typed one
  (`[italy]`), no line.
- **`--json` and the UIs' answer** (`reply::view`, read by the widget and
  Android): `result` stays the currency, and a new member lists the extra
  results, `"extra":[{"type":"shoes","results":["US 8.5 men","US 10 women"]}]`.
  The UIs draw the list from it.
- **Copy** stays the currency's (`47.15`, `42 EUR = 47.15 USD`).

### 4.2 No target: the device's region

As `12 usd` gives the local currency, a size with no target gives the **region's
system**. The region is the country of the device's time zone
(`time::device_zone` → `tz::zone_country`, the same reading as the local
currency):

| Country                 | Shoes | Kids' shoes | Women's clothing | Men's clothing | Bras |
|-------------------------|-------|-------------|------------------|----------------|------|
| US, LR, MM              | US    | US          | US               | US             | US   |
| UK, IE                  | UK    | UK          | UK               | UK             | UK   |
| Italy                   | EU    | EU          | IT               | IT             | IT   |
| FR, BE, ES, PT          | EU    | EU          | FR               | EU             | FR   |
| DE, AT, CH, NL, Nordics | EU    | EU          | DE               | EU             | EU   |
| Japan                   | JP    | JP          | JP               | JP             | JP   |
| China                   | CN    | CN          | CN               | CN             | CN   |
| anywhere else           | EU    | EU          | EU (= DE)        | EU             | EU   |

Kids' shoes have a column of their own (decided 2026-10-22), today with the same
values as Shoes.

When the size is already in the region's system (`shoe 42 eu` in Rome), the
result is that same size, and the details give the others. This is the "what is
this in other countries" answer.

### 4.3 The brand note

Every size answer adds the note `sizes vary by brand: check the brand's chart`,
shown under the result like the currency check note. It is short and always
there, because the user asked for it on every answer, even on one that already
names Nike and Clarks: those are two charts, not every brand.

### 4.4 Between two rows, and body measures

- A size **between rows** (`EU 42.3`), or a body measure, takes the **nearest
  row**. When it lies within 25% of the step from the midpoint, the note says
  `between EU 42 and 42.5`.
- **Two charts** (shoes; decided 2026-10-22):
  - both between the same rows: one note, as above;
  - only one between rows: its note, named, `between EU 40 and 41 (Clarks)`;
  - both between different rows: one note each, named:
    `between EU 41 and 42 (Nike)`, `between EU 41.5 and 42 (Clarks)`.
- **Exactly at the midpoint**, the larger row: the charts' own advice is to size
  up between sizes.
- **A value twice in a column** (Nike writes UK 6 for men's US 6.5 and US 7):
  both rows, as a range, `US 6.5–7`.
- A body measure **outside the chart** (a 35 cm foot): the last row, with the
  note `beyond the chart`.

## 5. Ambiguity (Aequalis asks, as everywhere)

| Situation                              | Question        | Choices                       |
|----------------------------------------|-----------------|-------------------------------|
| No person word, several charts ¹       | `whose sizes?`  | men · women · kids ²          |
| `man` or `clothes` for men, no item    | `which?`        | suit · shirt · letters        |
| Shirt: by collar or by letters         | `which sizing?` | collar (cm/in) · letters      |
| No such size in either chart           | error           | `no EU 52 in women's shoes` ³ |
| `eu` for clothing (FR/IT/DE differ)    | none: EU = DE   | note `EU sizes as in DE`      |

¹ Letters and `clothes`; shoes never ask, they show what is missing (§4.1).
² Each choice a full request. ³ Followed by the
range of both charts together, `(EU 34–44.5)`. A size in only one chart is
answered by that chart alone, named: `US 15.5 (Clarks)`.

**Dresses and bras** default to the women's chart, and **suits and collar
shirts** to the men's, without asking: their charts exist for one person only. A
note says which.

## 6. Charts

Each chart lists rows. A row is one size in every system, and the body measure
it stands for. Sources are cited per chart. Where they disagree, the spec says
which was chosen.

### 6.1 Shoes (phase 1)

Shoes have **no exact conversion**. UK and US sizes step by a barleycorn (⅓ in),
EU sizes by a Paris point (⅔ cm, the last's length × 1.5), and the zero points
differ between men, women and children
([BSI](https://knowledge.bsigroup.com/products/footwear-sizing-conversion-of-sizing-systems),
[ISO 9407
Mondopoint](https://webstore.ansi.org/preview-pages/ISO/preview_ISO+9407-2019.pdf)).

#### Two charts: Nike and Clarks (decided)

Retail charts fall into two families, which put the same EU number on different
UK and US sizes. Aequalis holds one chart of each, from the brand's own size
guide (read 2026-10-22):

- **Nike**, the sports brands' chart:
  [men's](https://www.nike.com/size-fit/mens-footwear),
  [women's](https://www.nike.com/size-fit/womens-footwear),
  [kids'](https://www.nike.com/size-fit/kids-footwear). It has JP and the foot
  length.
- **Clarks**, the traditional British chart:
  [clarks.com.au](https://clarks.com.au/pages/size-guide) (UK, EU, US; foot
  length for women and kids),
  [clarksstores.ae](https://www.clarksstores.ae/pages/fitguide) (the men's foot
  lengths; its UK and EU match the Australian chart). Its EU numbers sit about
  half a size lower: men's EU 42 is UK 7.5 / US 8.5 at Nike, UK 8 / US 9 at
  Clarks.

How they answer:

- **Each chart answers on its own**, by the nearest row (§4.4).
- **Same size:** the size alone, `US 8`.
- **Different sizes:** both, each named once: `US 8.5 (Nike), 9 (Clarks)`. Never
  more than two values, never more than one name each.
- **A size in one chart only** (EU 50 men's is past Nike's last row here): that
  chart alone, still named, `US 15.5 (Clarks)`.
- **A system one chart lacks** (Clarks has no JP, CN reads EU): the other chart
  alone.
- **The details line** is Nike's (§4.1).

Left out, on purpose:

- **Adidas.** It labels EU sizes in thirds (42⅔), its UK size is US − ½ (Nike's
  is US − 1), and its women's EU 42 is US 9.5 (Nike US 10, Clarks US 10.5). It
  matches neither chart, and naming it would promise values it doesn't have. An
  Adidas label typed is read as a number like any other: `42⅔ eu men shoes us` →
  `US 9 (Nike), 9.5 (Clarks)`, Adidas's own US 9 among them ([Adidas's
  chart](http://www.sport-conrad.com/en/brands/adidas/size-chart/)).
- **Brannock.** It isn't a chart. It's the US device that measures a foot for a
  US size, and both charts' US sizes follow its foot lengths: a men's US 9 is a
  26.2 cm foot at Nike, 26.3 cm at Clarks.

**JP** is Nike's CM/JP column, the cm on the box and on Japanese labels. It is
**not** the foot length: Nike's chart gives both, and a men's US 9 is JP 27 for
a 26.2 cm foot. **CN = the EU number** (as Chinese shops label shoes), so it has
no column of its own: `42 cn shoe` is EU 42, with a note.

**From the foot:** the foot-length column of each chart, not JP. Inches are ×
2.54.

- `27 cm men foot` in Rome: Nike's nearest row is 10 11/16 in = 27.15 cm (US 10,
  EU 44), Clarks's 272 mm (UK 9, US 10, EU 43). The result is
  `EU 44 (Nike), 43 (Clarks)`.
- `11 in men foot` (27.94 cm): Nike's 11 in row and Clarks's 280 mm row are both
  US 11, so the result is `US 11`.

#### Men's

##### Nike

| US   | UK   | EU   | JP (cm) | Foot (in) |
|------|------|------|---------|-----------|
| 6    | 5.5  | 38.5 | 24      | 9 5/16    |
| 6.5  | 6    | 39   | 24.5    | 9 1/2     |
| 7    | 6    | 40   | 25      | 9 11/16   |
| 7.5  | 6.5  | 40.5 | 25.5    | 9 13/16   |
| 8    | 7    | 41   | 26      | 10        |
| 8.5  | 7.5  | 42   | 26.5    | 10 3/16   |
| 9    | 8    | 42.5 | 27      | 10 5/16   |
| 9.5  | 8.5  | 43   | 27.5    | 10 1/2    |
| 10   | 9    | 44   | 28      | 10 11/16  |
| 10.5 | 9.5  | 44.5 | 28.5    | 10 13/16  |
| 11   | 10   | 45   | 29      | 11        |
| 11.5 | 10.5 | 45.5 | 29.5    | 11 3/16   |
| 12   | 11   | 46   | 30      | 11 5/16   |
| 12.5 | 11.5 | 47   | 30.5    | 11 1/2    |
| 13   | 12   | 47.5 | 31      | 11 11/16  |
| 13.5 | 12.5 | 48   | 31.5    | 11 13/16  |
| 14   | 13   | 48.5 | 32      | 12        |
| 14.5 | 13.5 | 49   | 32.5    | 12 3/16   |
| 15   | 14   | 49.5 | 33      | 12 5/16   |

UK 6 twice is Nike's own (§4.4: `uk 6` → `US 6.5–7`).

##### Clarks

| UK   | US   | EU   | Foot (mm) |
|------|------|------|-----------|
| 5    | 6    | 38   | 237       |
| 5.5  | 6.5  | 39   | 241       |
| 6    | 7    | 39.5 | 246       |
| 6.5  | 7.5  | 40   | 250       |
| 7    | 8    | 41   | 254       |
| 7.5  | 8.5  | 41.5 | 258       |
| 8    | 9    | 42   | 263       |
| 8.5  | 9.5  | 42.5 | 267       |
| 9    | 10   | 43   | 272       |
| 9.5  | 10.5 | 44   | 276       |
| 10   | 11   | 44.5 | 280       |
| 10.5 | 11.5 | 45   | 285       |
| 11   | 12   | 46   | 289       |
| 11.5 | 12.5 | 46.5 | 293       |
| 12   | 13   | 47   | 298       |
| 12.5 | 13.5 | 47.5 | 302       |
| 13   | 14   | 48   | 306       |
| 13.5 | 14.5 | 49   | 310       |
| 14   | 15   | 49.5 | 314       |
| 14.5 | 15.5 | 50   | 319       |
| 15   | 16   | 51   | 323       |

#### Women's

##### Nike

| US   | UK  | EU   | JP (cm) | Foot (in) ¹ |
|------|-----|------|---------|-------------|
| 5    | 2.5 | 35.5 | 22      | 8 1/2       |
| 5.5  | 3   | 36   | 22.5    | 8 11/16     |
| 6    | 3.5 | 36.5 | 23      | 8 13/16     |
| 6.5  | 4   | 37.5 | 23.5    | 9           |
| 7    | 4.5 | 38   | 24      | 9 3/16      |
| 7.5  | 5   | 38.5 | 24.5    | 9 5/16      |
| 8    | 5.5 | 39   | 25      | 9 1/2       |
| 8.5  | 6   | 40   | 25.5    | 9 11/16     |
| 9    | 6.5 | 40.5 | 26      | 9 13/16     |
| 9.5  | 7   | 41   | 26.5    | 10          |
| 10   | 7.5 | 42   | 27      | 10 3/16     |
| 10.5 | 8   | 42.5 | 27.5    | 10 5/16     |
| 11   | 8.5 | 43   | 28      | 10 1/2      |
| 11.5 | 9   | 44   | 28.5    | 10 11/16    |
| 12   | 9.5 | 44.5 | 29      | 10 13/16    |

¹ **To check on nike.com before release.** The page's text loses the table, and
its foot column is shorter than the size columns. These values line up with the
men's chart on the same EU size (women's US 10 and men's US 8.5 are both EU 42,
both 10 3/16 in), which is Brannock's women's = men's + 1.5.

##### Clarks

| UK  | US   | EU   | Foot (mm) |
|-----|------|------|-----------|
| 2   | 4.5  | 34   | 212       |
| 2.5 | 5    | 35   | 216       |
| 3   | 5.5  | 35.5 | 221       |
| 3.5 | 6    | 36   | 225       |
| 4   | 6.5  | 37   | 229       |
| 4.5 | 7    | 37.5 | 233       |
| 5   | 7.5  | 38   | 237       |
| 5.5 | 8    | 39   | 241       |
| 6   | 8.5  | 39.5 | 246       |
| 6.5 | 9    | 40   | 250       |
| 7   | 9.5  | 41   | 254       |
| 7.5 | 10   | 41.5 | 258       |
| 8   | 10.5 | 42   | 263       |
| 8.5 | 11   | 42.5 | 267       |
| 9   | 11.5 | 43   | 272       |

#### Kids'

US toddler sizes end in `C`, youth sizes in `Y`. Kids' UK sizes, and a US
number written without its letter, start over: UK 4.5 is a toddler's 5C and a
youth's 5Y. Such a size gives both, labelled, `US 5C toddler, US 5Y youth`,
unless a toddler or youth word says which (§2.4) (decided 2026-10-22).

##### Nike ²

| US    | UK   | EU   | JP (cm) | Foot (in) |
|-------|------|------|---------|-----------|
| 4C    | 3.5  | 19.5 | 10      | 4 9/16    |
| 5C    | 4.5  | 21   | 11      | 4 15/16   |
| 6C    | 5.5  | 22   | 12      | 5 1/4     |
| 7C    | 6.5  | 23.5 | 13      | 5 9/16    |
| 8C    | 7.5  | 25   | 14      | 5 15/16   |
| 9C    | 8.5  | 26   | 15      | 6 1/4     |
| 10C   | 9.5  | 27   | 16      | 6 9/16    |
| 10.5C | 10   | 27.5 | 16.5    | 6 3/4     |
| 11C   | 10.5 | 28   | 17      | 6 15/16   |
| 11.5C | 11   | 28.5 | 17.5    | 7 1/16    |
| 12C   | 11.5 | 29.5 | 18      | 7 1/4     |
| 12.5C | 12   | 30   | 18.5    | 7 7/16    |
| 13C   | 12.5 | 31   | 19      | 7 9/16    |
| 13.5C | 13   | 31.5 | 19.5    | 7 3/4     |
| 1Y    | 13.5 | 32   | 20      | 7 15/16   |
| 1.5Y  | 1    | 33   | 20.5    | 8 1/16    |
| 2Y    | 1.5  | 33.5 | 21      | 8 1/4     |
| 2.5Y  | 2    | 34   | 21.5    | 8 7/16    |
| 3Y    | 2.5  | 35   | 22      | 8 9/16    |
| 3.5Y  | 3    | 35.5 | 22.5    | 8 3/4     |
| 4Y    | 3.5  | 36   | 23      | 8 13/16   |
| 4.5Y  | 4    | 36.5 | 23.5    | 8 15/16   |
| 5Y    | 4.5  | 37.5 | 23.5    | 9 1/8     |
| 5.5Y  | 5    | 38   | 24      | 9 1/4     |
| 6Y    | 5.5  | 38.5 | 24      | 9 3/8     |
| 6.5Y  | 6    | 39   | 24.5    | 9 9/16    |
| 7Y    | 6    | 40   | 25      | 9 11/16   |

² **To check on nike.com before release**, as the women's: the page's text loses
the table. The UK column is half a size above the one this spec had before (4C
was UK 3, now 3.5, as Nike writes it).

##### Clarks ³

| UK   | US    | EU   | Foot (mm) |
|------|-------|------|-----------|
| 3.5  | 4C    | 19   | 114       |
| 4    | 4.5C  | 20   | 119       |
| 4.5  | 5C    | 20.5 | 123       |
| 5    | 5.5C  | 21   | 127       |
| 5.5  | 6C    | 22   | 131       |
| 6    | 6.5C  | 22.5 | 135       |
| 6.5  | 7C    | 23   | 140       |
| 7    | 7.5C  | 24   | 144       |
| 7.5  | 8C    | 25   | 148       |
| 8    | 8.5C  | 25.5 | 152       |
| 8.5  | 9C    | 26   | 157       |
| 9    | 9.5C  | 27   | 161       |
| 9.5  | 10C   | 27.5 | 165       |
| 10   | 10.5C | 28   | 169       |
| 10.5 | 11C   | 28.5 | 174       |
| 11   | 11.5C | 29   | 178       |
| 11.5 | 12C   | 29.5 | 182       |
| 12   | 12.5C | 30   | 186       |
| 12.5 | 13C   | 31   | 190       |
| 13   | 13.5C | 32   | 195       |
| 13.5 | 1Y    | 32.5 | 199       |
| 1    | 1.5Y  | 33   | 203       |
| 1.5  | 2Y    | 33.5 | 207       |
| 2    | 2.5Y  | 34   | 212       |
| 2.5  | 3Y    | 35   | 216       |
| 3    | 3.5Y  | 35.5 | 220       |
| 3.5  | 4Y    | 36   | 224       |
| 4    | 5Y    | 37   | 229       |
| 4.5  | 5.5Y  | 37.5 | 233       |
| 5    | 6Y    | 38   | 237       |
| 5.5  | 6.5Y  | 39   | 242       |
| 6    | 7Y    | 39.5 | 246       |
| 6.5  | 7.5Y  | 40   | 250       |

³ Clarks's chart writes the bare US number. `C` and `Y` are added here as US
labels write them, so that US 4C (EU 19) and US 4Y (EU 36) stay apart. US 4Y →
5Y with no 4.5Y is as published.

### 6.2 Women's clothing (phase 2)

Dresses, tops and trousers share one numeric chart. The offsets most sources
agree on
([size.ly](https://www.size.ly/conversion-charts/international-size-charts-women),
[ASOS](https://www.asos.com/discover/size-charts/women/dresses/)) are:

- UK = US + 4;
- DE (EU) = US + 30;
- FR = US + 32;
- IT = US + 36.

Sources **disagree on FR and IT** (some put UK 8 = FR 38 = IT 40, others FR 40 =
IT 44). The common offsets above are used (decided):
`IT 44 = FR 40 = DE 38 = UK 12 = US 8`. Every answer lists both IT and FR among
the details (from a US size: `IT 44 · FR 40`), with no second, one-step-smaller
mapping. The brand note covers the rest. JP = US + 3 (decided: US 8 = JP 11;
Japanese sizes are odd numbers, 5, 7, 9, 11…).

| US  | UK  | DE/EU | FR  | IT  | JP  | INT  | CN       |
|-----|-----|-------|-----|-----|-----|------|----------|
| 0   | 4   | 30    | 32  | 36  | 3   | XXS  | 150/76A  |
| 2   | 6   | 32    | 34  | 38  | 5   | XS   | 155/80A  |
| 4   | 8   | 34    | 36  | 40  | 7   | S    | 155/84A  |
| 6   | 10  | 36    | 38  | 42  | 9   | S/M  | 160/88A  |
| 8   | 12  | 38    | 40  | 44  | 11  | M    | 165/92A  |
| 10  | 14  | 40    | 42  | 46  | 13  | L    | 165/96A  |
| 12  | 16  | 42    | 44  | 48  | 15  | L/XL | 170/100A |
| 14  | 18  | 44    | 46  | 50  | 17  | XL   | 170/104A |
| 16  | 20  | 46    | 48  | 52  | 19  | XXL  | 175/110A |
| 18  | 22  | 48    | 50  | 54  | 21  | XXL  | 175/116A |

**Body measures are not supported in this version** (decided): `92 cm bust` and
the like are left for later, since sources disagree on them by 2–4 cm.

CN women's: the Chinese GB/T 1335 label is height/bust/body type (`160/84A`),
supported in phase 2. A label is read by its bust (the number after `/`) and
written as the row's label above.

### 6.3 Men's clothing (phase 2)

**Suits and jackets:** chest in inches (US/UK), EU/IT/FR = US + 10.

| US/UK (in) | EU/IT/FR | JP  | INT |
|------------|----------|-----|-----|
| 34         | 44       | S   | XS  |
| 36         | 46       | S   | S   |
| 38         | 48       | M   | M   |
| 40         | 50       | L   | L   |
| 42         | 52       | L   | L   |
| 44         | 54       | LL  | XL  |
| 46         | 56       | LL  | XL  |
| 48         | 58       | 3L  | XXL |
| 50         | 60       | 3L  | XXL |

**Shirts by collar:** cm (EU) and inches (US/UK), the nearest half inch.

| EU (cm) | US/UK (in) | INT |
|---------|------------|-----|
| 36      | 14         | XS  |
| 37      | 14.5       | S   |
| 38      | 15         | S   |
| 39      | 15.5       | M   |
| 40      | 15.75      | M   |
| 41      | 16         | L   |
| 42      | 16.5       | L   |
| 43      | 17         | XL  |
| 44      | 17.5       | XL  |
| 45      | 18         | XXL |
| 46      | 18.5       | XXL |

**Letters:** the men's are the INT column of the suits chart, the women's the
INT column of §6.2.

### 6.4 Kids' clothing (phase 3)

Kids' sizes go by **height in cm** (EU/DE/FR/IT/CN), or by **age** (US: months,
`2T`–`5T`, then years; UK: age ranges).

| Height (EU) | US   | UK     |
|-------------|------|--------|
| 56          | 0–3M | 0–3m   |
| 62          | 3–6M | 3–6m   |
| 68          | 6M   | 6–9m   |
| 74          | 9M   | 9–12m  |
| 80          | 12M  | 12–18m |
| 86          | 18M  | 18–24m |
| 92          | 2T   | 2–3y   |
| 98          | 3T   | 3–4y   |
| 104         | 4T   | 4–5y   |
| 110         | 5    | 5–6y   |
| 116         | 6    | 6–7y   |
| 122         | 7    | 7–8y   |
| 128         | 8    | 8–9y   |
| 140         | 10   | 9–10y  |
| 152         | 12   | 11–12y |
| 164         | 14   | 13–14y |
| 176         | 16   | 15–16y |

### 6.5 Bras (phase 3)

**Band:**

| US/UK (in) | EU/DE (cm) | FR/ES (cm) | IT  |
|------------|------------|------------|-----|
| 30         | 65         | 80         | 1   |
| 32         | 70         | 85         | 2   |
| 34         | 75         | 90         | 3   |
| 36         | 80         | 95         | 4   |
| 38         | 85         | 100        | 5   |
| 40         | 90         | 105        | 6   |
| 42         | 95         | 110        | 7   |
| 44         | 100        | 115        | 8   |

**Cups:** they agree up to D, then differ.

| US    | UK  | EU/FR/IT |
|-------|-----|----------|
| A     | A   | A        |
| B     | B   | B        |
| C     | C   | C        |
| D     | D   | D        |
| DD/E  | DD  | E        |
| DDD/F | E   | F        |
| G     | F   | G        |
| H     | FF  | H        |

Cups beyond DD vary between US brands: the row above is used (decided), with the
brand note.

### 6.6 Hats (phase 3)

**EU = head circumference in cm.** US = circumference in inches ÷ π, to the
nearest ⅛. UK = US − ⅛.

| EU (cm) | US  | UK  | INT |
|---------|-----|-----|-----|
| 54      | 6¾  | 6⅝  | XS  |
| 55      | 6⅞  | 6¾  | S   |
| 56      | 7   | 6⅞  | S   |
| 57      | 7⅛  | 7   | M   |
| 58      | 7¼  | 7⅛  | M   |
| 59      | 7⅜  | 7¼  | L   |
| 60      | 7½  | 7⅜  | L   |
| 61      | 7⅝  | 7½  | XL  |
| 62      | 7¾  | 7⅝  | XL  |

## 7. UI

- **A Sizes menu item** (decided 2026-10-22), right after Currencies:
  - **TUI:** `F4 󰩻󱗈 Sizes`, Alt+Z; Trivia moves from F4 to F6 (Alt+T kept).
    Every toolbar entry has a Nerd Font symbol (Material Design, monochrome in
    the terminal's colours; decided 2026-10-22, in place of the emoji 👕👟):
    󰑭 Measures, 󰄖 Currencies, 󰩻󱗈 Sizes (md-tshirt_crew and md-shoe_sneaker),
    󰋚 History, 󰌶 Trivia, 󰘜 Keys, 󰢻 Settings. Symbol and name with room for
    the online indicator after them (from 100 columns); else the key and its
    symbol (from 44 columns); else the keys alone.
  - **Android:** in the toolbar after Currencies, with the Material Symbols
    `apparel` (a T-shirt) and `steps` (footprints: Material Symbols has no
    shoe) icons, as paths in `Icons.kt`. The Sizes key shows or hides the
    Sizes' keys (below); on a large screen, the panel beside the keys. The
    table (`shoes 42`) is a line per person on the phone, the person in bold
    and the sizes after it:
    `Men  US 8.5 · UK 7.5 · EU 42 · JP 26.5 · CN 42`. The keyboard collapses
    (its ⌄ key, a swipe down) to give its room to the answer; a bar brings it
    back (a tap, a swipe up) (decided 2026-10-22). ⏎ on an answer taller than
    its room collapses it too, once for that request; typing never does. In
    landscape and in portrait the Sizes take the letters' place as big keys
    (Shoes, the systems, ABC back to the letters, ⌫, space, ⏎), the digits
    beside them (landscape) or above them with the operators (portrait); in the
    calculator keys (no letters) a strip above them (decided 2026-10-22).
  - **Omarchy widget:** no Sizes item for now; it shows size answers as any
    other.
  - **It opens** the groups (Shoes; later Women, Men, Kids, Bras, Hats, each in
    its phase), which write their item word, and the system keys, which write
    their code.
  - **A system key brings `shoes`** (decided 2026-10-22): written after the
    request, and `shoes` at its start when the request has no item word yet,
    so that it asks for sizes, not a currency: `42` + EU → `shoes 42 eu`, an
    empty request + EU → `shoes eu`. A group's word is not written twice.
- **Measures panel:** the same Sizes class there too, beside Length, Mass and
  the others (TUI, widget, Android).
- **The systems are the tabs:** EU, UK, US, IT, FR, DE, JP, CN, INT. A key
  writes its system code; a group writes its item word in the device's language
  when known, else English.
- **Trivia: a Shoes section** (decided 2026-10-22), with three conversion
  tables, Men, Women and Kids, from Nike's chart (§6.1): US, UK, EU, JP and the
  foot length in cm (Nike's inches × 2.54, one decimal: `25.9 cm`). Three
  entries, Men's, Women's and Kids' shoes; the popup (the detail on Android)
  shows the table, and a row writes its request (`42 eu men shoes`).
- **Suggestions while typing:** the item, person and body words of the 10
  languages (`scar` → `scarpe - shoes`), and the system codes after a size in a
  size request (`shoes 42` and a space → `eu, uk, us…`).
- **Beside a currency:** the list (§4.1), from the answer's `extra` member.
- **Android:** the Sizes class in the Measures panel; the quick window answers
  size requests as any other.
- **Copy:** the value (`8.5`, the first chart's) or the expression
  (`EU 42 = US 8.5 (Nike), 9 (Clarks)`).

## 8. Phases

Each phase is released as a `minor`.

1. **Shoes:**
   - men, women and kids, in all systems, from the foot too;
   - Nike's and Clarks's charts, named in the result where they differ (§6.1);
   - the women's and kids' Nike foot lengths checked on nike.com (§6.1 ¹ ²);
   - currency as the default, with the shoes line beside it (§4.1);
   - the words of every item in the 10 languages (later items are recognised,
     not misread, and answered with the error `only shoes for now`);
   - the Sizes class with its Shoes group, in Measures and as its own menu item
     (§7);
   - the Shoes section in Trivia.
2. **Women's and men's clothing:** dresses, tops and trousers, suits, collar
   shirts, the letters, the CN `160/84A` labels, a person word alone
   (`12 US woman EU`).
3. **Kids' clothing, bras, hats.**

Each phase:

- its charts in `src/data/sizes.json` (compiled by `build/tables.rs`);
- its rules in `src/sizes.rs`;
- its test cases (§9) as `tests/sizes.rs`, on the simulated `italy` and `us`
  machines;
- tried on the phone before release.

## 9. Test cases

`[italy]` and `[us]` are the simulated machines. **→** is the result, and the
details and notes are shown where they matter.

### Phase 1

| Request                 | Machine | Expected                                                                                                   |
|-------------------------|---------|------------------------------------------------------------------------------------------------------------|
| `42 eu men shoes us`    | any     | `US 8.5 (Nike), 9 (Clarks)` ¹                                                                              |
| `41 eu men shoes us`    | any     | `US 8` (the charts agree)                                                                                  |
| `42 eu shoes us`        | any     | `US 8.5 men, US 10 women` (no question)                                                                    |
| `30 eu shoes us`        | any     | `US 12.5C kids`                                                                                            |
| `50 eu shoes us`        | any     | `US 15.5 men (Clarks)`                                                                                     |
| `42 eu shoes`           | any     | the table: men `US 8.5 · UK 7.5 · EU 42 · JP 26.5 · CN 42`, women `US 10 · UK 7.5 · EU 42 · JP 27 · CN 42` |
| `42 eu donna scarpe us` | any     | `US 10 (Nike), 10.5 (Clarks)`                                                                              |
| `scarpe uomo 42`        | italy   | `EU 42`; details `US 8.5 · UK 7.5 · JP 26.5 cm · CN 42`                                                    |
| `men shoes 9 us`        | italy   | `EU 42.5 (Nike), 42 (Clarks)`                                                                              |
| `men shoes 42 eu`       | us      | `US 8.5 (Nike), 9 (Clarks)`                                                                                |
| `uk 6 men shoes us`     | any     | `US 6.5–7 (Nike), 7 (Clarks)`                                                                              |
| `27 cm men foot`        | italy   | `EU 44 (Nike), 43 (Clarks)`                                                                                |
| `11 in men foot`        | us      | `US 11` (27.94 cm: both charts' US 11)                                                                     |
| `zapatos mujer 38`      | italy   | `EU 38`                                                                                                    |
| `42.2 eu men shoes us`  | any     | `US 8.5 (Nike), 9 (Clarks)`, note `between EU 42 and 42.5`                                                 |
| `40.5 eu men shoes us`  | any     | `US 7.5 (Nike), 8 (Clarks)`, note `between EU 40 and 41 (Clarks)`                                          |
| `41.75 eu men shoes us` | any     | `US 8.5 (Nike), 9 (Clarks)` ²                                                                              |
| `42⅔ eu men shoes us`   | any     | `US 9 (Nike), 9.5 (Clarks)` (an Adidas label)                                                              |
| `50 eu men shoes us`    | any     | `US 15.5 (Clarks)` (past Nike's last row)                                                                  |
| `60 eu men shoes us`    | any     | error `no EU 60 in men's shoes (EU 38–51)`                                                                 |
| `42 eu nike shoes us`   | any     | not a size request: brands are not words                                                                   |
| `42 eu us`              | any     | list: `42 EUR = … USD`, `shoes: 42 eu = US 8.5 men, US 10 women` ³                                         |
| `42 eu`                 | us      | list: `42 EUR = … USD` (the local currency), `shoes: 42 eu = US 8.5 men, US 10 women`                      |
| `42 eu`                 | italy   | the currency hint alone (the region's system is EU)                                                        |
| `42.2 eu us`            | any     | list, `shoes: 42.2 eu = US 8.5 men, US 10 women` (no notes)                                                |
| `36 eu us`              | any     | list, `shoes: 36 eu = US 5.5 women, US 4Y kids`                                                            |
| `30 eu us`              | any     | list, `shoes: 30 eu = US 12.5C kids`                                                                       |
| `47 eu us`              | any     | list, `shoes: 47 eu = US 12.5 men`                                                                         |
| `50 eu us`              | any     | list, `shoes: 50 eu = US 15.5 men (Clarks)`                                                                |
| `42 eu fr`              | any     | unchanged: the error `unknown unit` (`fr` is no currency code)                                             |
| `100 eu us`             | any     | unchanged: EUR → USD alone (100 is in no shoe chart)                                                       |
| `42 eu jpy`             | any     | unchanged: EUR → JPY alone (`jpy` is no size system)                                                       |
| `42 in`                 | any     | unchanged: a length, asks for the target                                                                   |
| `12 us woman eu`        | any     | error `only shoes for now`                                                                                 |
| `44 it dress us`        | any     | error `only shoes for now`                                                                                 |
| `42 it men shoes us`    | any     | `US 8.5 (Nike), 9 (Clarks)`, note `Italian shoe sizes are EU sizes`                                        |
| every answer            |         | note `sizes vary by brand: check the brand's chart`                                                        |

¹ With the details `UK 7.5 · EU 42 · JP 26.5 cm · CN 42` (Nike's; every system
but the result's, §4.1) and the note `men's shoes (details: Nike)`.

² With the notes `between EU 41 and 42 (Nike)` and
`between EU 41.5 and 42 (Clarks)`.

³ `--json`: `"result":"… USD"` and
`"extra":[{"type":"shoes","results":["US 8.5 men","US 10 women"]}]`.

### Phase 2

| Request            | Machine | Expected                                            |
|--------------------|---------|-----------------------------------------------------|
| `IT 44 dress US`   | any     | `US 8`; details `UK 12 · FR 40 · DE 38 · JP 11 · M` |
| `vestito 44`       | italy   | `IT 44` (a dress: women's chart, no question)       |
| `us 8 dress`       | italy   | `IT 44`                                             |
| `M women top it`   | any     | `IT 44`                                             |
| `12 US woman EU`   | any     | `EU 42` (a person word alone: clothing)             |
| `42 eu women us`   | any     | `US 12` (women's clothing, not shoes)               |
| `40 US man EU`     | any     | question `which?`: suit · shirt · letters           |
| `40 us suit eu`    | any     | `EU 50`                                             |
| `39 cm collar`     | us      | `US 15.5`                                           |
| `40 us suit int`   | any     | `L`                                                 |
| `160/92A dress us` | any     | `US 8`                                              |

### Phase 3

| Request        | Machine | Expected                          |
|----------------|---------|-----------------------------------|
| `128 kids`     | us      | `US 8` (a person word: clothing)  |
| `4T kids eu`   | any     | `EU 104`                          |
| `75B bra us`   | any     | `US 34B`                          |
| `57 cm hat us` | any     | `US 7⅛`                           |

## 10. Decisions

All settled:

- Sizes only when asked: an item, body or person word. Currency stays the
  default (2026-10-22).
- Beside a currency that could be a shoe size: a list, the currency then a
  shoes line (men, women, kids: whoever has the size; Nike's, Clarks's named
  when only it has it; no notes), on the CLI's stdout too; `extra` in `--json`
  and the UIs' answer; Copy the currency's (2026-10-22).
- The "between" note names its chart when the charts differ (2026-10-22).
- Clothing in phase 1: the error `only shoes for now` (2026-10-22).
- Kids' shoes: a region column of their own, today as Shoes (2026-10-22).
- Shoes never ask whose: no person shows whoever has the size; no target the
  region's system then the others; neither a table, lined up on the decimal
  point (2026-10-22).
- A Sizes menu item after Currencies (TUI F4 󰩻󱗈 Alt+Z, Nerd Font symbols,
  Trivia to F6; Android `apparel` + `steps`; not in the widget yet), and still
  a class in Measures; a Shoes section in Trivia, Nike's tables (2026-10-22).
- A person word alone means clothing: women's and kids' answer, men's asks
  which.
- Shoes: Nike's and Clarks's charts, each named in the result where they differ,
  one name each, two values at most (2026-10-22). Adidas and Brannock are not
  named (§6.1).
- From the foot: the charts' foot-length column, not JP (JP is the box's cm).
- CN shoes = the EU number.
- The common FR/IT mapping, both shown.
- US 8 = JP 11.
- CN `160/84A` labels in phase 2.
- The bra cups as proposed.
- No body measures for clothing, no jeans and no rings in this version.
