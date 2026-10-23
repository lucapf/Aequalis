# Clothing sizes — specification

Status: **draft for review** (23 Oct 2026). Every decision is in §10; coding
starts once this spec is agreed. Shoes are in `docs/SPEC_SIZES.md`, whose
vocabulary (§2), region table (§4.2) and "between" rule (§4.4) this spec reuses.

## 1. Scope

Aequalis converts clothing sizes between countries' systems, from a letter to a
number and back, and from a body measure to a size, the way it converts shoes:

- `44 it dress us` → `US 8`
- `clothes M it` → `IT 44 women, IT 48 men's suit, IT 48–49 men's trousers`
- `92 cm bust` → `UK 12` (in Dublin)
- `42 eu us` → the currency, then a shoes line, then a clothes line

**The same strategy as shoes** (decided 2026-10-23):

- a clothes word makes a clothes request (§3.1);
- without one, a request that could also be a clothes size keeps its answer
  and gets a **clothes line** in the additional results, as the shoes line
  beside a currency (§3.3);
- a request that means several things answers **every reading, by priority**,
  never a question (§3.4).

**The charts, in priority order** (decided 2026-10-23):

1. **Women's**: dresses, tops and trousers share one chart (§6.1).
2. **Men's suits and jackets**, by chest, with the men's letters (§6.2).
3. **Men's trousers**, by waist (§6.3).
4. **Men's shirts**, by collar (§6.4).

The last two are technical charts: they always come last.

**Common tables, no brands** (decided 2026-10-23). Clothing brands differ too
much for two charts to cover them, so each system has one value, from the
offsets most retail tables agree on, and every answer carries the brand note
(`sizes vary by brand: check the brand's chart`, SPEC_SIZES §4.3).

**Body measures for every chart** (decided 2026-10-23): bust, waist, hip and
chest, in cm or inches, from the body measures ASOS prints for EN 13402 sizing
(§6). A measure takes the nearest row, with the "between" note.

Non-goals:

- **Kids' clothing** stays in phase 3 (SPEC_SIZES §8). Its words are recognised
  and answered `kids' clothes: not yet`; a request without a person never lists
  kids.
- **Bras and hats** stay in phase 3.
- **Natural phrasing for every request** (`tell me how many km are 10 mi`) has a
  spec of its own, after this one (TODO `nl-1`). Clothes accept only the size
  fillers of §2.4.
- **No fit advice**, no garment measures (lengths, sleeves).

## 2. Vocabulary

### 2.1 Systems

| Code  | Women's | Men's suit | Men's trousers | Men's shirt | Written      |
|-------|---------|------------|----------------|-------------|--------------|
| `us`  | yes     | chest (in) | waist (in)     | collar (in) | `US 8`       |
| `uk`  | yes     | chest (in) | waist (in)     | collar (in) | `UK 12`      |
| `eu`  | = `de`  | yes        | yes            | collar (cm) | `EU 38`      |
| `de`  | yes     | = `eu`     | = `eu`         | = `eu`      | `DE 38`      |
| `fr`  | yes     | = `eu`     | = `eu`         | = `eu`      | `FR 40`      |
| `it`  | yes     | = `eu`     | = `eu`         | none        | `IT 44`      |
| `jp`  | yes     | letters    | none           | none        | `JP 11`      |
| `cn`  | label   | none       | none           | none        | `CN 165/92A` |
| `int` | letters | letters    | letters        | letters     | `M`          |

- **`eu` for women's means DE**, the EN 13402 body-based numbering, with the
  note `EU sizes as in DE` (as SPEC_SIZES §2.1 decided).
- **Men's suits and trousers** use one continental number: `eu`, `it`, `fr` and
  `de` all read and write it (`IT 50`, `EU 50`).
- A system a chart lacks is skipped for that chart (`44 it collar` has no
  answer; `44 it` reaches the other charts).

### 2.2 Labels

| Label           | Examples                                    | Charts                 |
|-----------------|---------------------------------------------|------------------------|
| number          | `44`, `8`, `15.5`, `15½`, `15,5`            | all                    |
| attached system | `IT44`, `44IT`, `EU42`, `US8`, `UK12`       | all                    |
| letters         | `XXS`, `XS`, `S`, `M`, `L`, `XL`, `XXL`, `2XL`, `XXXL`, `3XL` | all  |
| JP letters      | `S`, `M`, `L`, `LL`, `3L`                   | men's suit (with `jp`) |
| range           | `S/M`, `S-M`, `L/XL`, `8-10`, `8/10`        | all                    |
| CN label        | `160/84A`, `165/92A`, `165/92` (A optional) | women's                |
| waist           | `W32`, `32W`, `W32 L34`, `32x34`, `32/34`   | men's trousers         |

- **Letters are case-free** (`m`, `xl`); `2XL` = `XXL`, `3XL` = `XXXL`.
- **A letter that labels several rows** (women's `XXL` is US 16 and 18) reads
  as both rows and answers a range (§4.4).
- **A range** reads both ends and answers a range; `S/M` is first the row
  labelled `S/M`, else S to M.
- **CN labels** are read by the number after `/` (the bust), as SPEC_SIZES §6.2
  decided; the height before it is shown, not used.
- **The inseam** (`L34`, the second number of `32x34`) is ignored with the note
  `inseam L34 has no EU size` (decided 2026-10-23).

### 2.3 Item and body words

The item words are SPEC_SIZES §2.3's, in the same 10 languages (en, es, pt, fr,
de, id, tr, vi, it, pl), accents optional, singular and plural:

| Item word                      | Charts it reaches                       |
|--------------------------------|-----------------------------------------|
| clothes (any clothing)         | all four                                |
| dress                          | women's                                 |
| top / T-shirt                  | women's, men's suit (its letters)       |
| trousers / pants               | women's, men's trousers                 |
| suit / jacket / blazer         | men's suit                              |
| shirt                          | men's shirt, men's suit (its letters)   |

New body words (the measure decides the charts, decided 2026-10-23):

| Measure      | Charts                   | en          | es        | pt           | fr          | de          |
|--------------|--------------------------|-------------|-----------|--------------|-------------|-------------|
| bust         | women's                  | bust        | busto     | busto        | poitrine    | Brust(umfang) |
| chest        | men's suit               | chest       | pecho     | peito        | torse       | Brustumfang |
| waist        | women's, men's trousers  | waist       | cintura   | cintura      | taille      | Taille, Bund |
| hip          | women's                  | hip(s)      | cadera(s) | quadril      | hanches     | Hüfte       |
| collar, neck | men's shirt              | (SPEC_SIZES §2.3) |    |              |             |             |

| Measure      | id            | tr       | vi        | it              | pl             |
|--------------|---------------|----------|-----------|-----------------|----------------|
| bust         | lingkar dada  | göğüs    | vòng ngực | seno            | biust          |
| chest        | dada          | göğüs    | ngực      | torace, petto   | klatka piersiowa |
| waist        | pinggang      | bel      | eo        | vita            | talia, pas     |
| hip          | pinggul       | kalça    | hông      | fianchi         | biodra         |

- **Clashes:** `taille` is French for waist and for size: as a filler (§2.4) it
  is ignored, as a measure it needs a number with `cm` or `in`
  (`70 cm taille`). `göğüs` (bust and chest) reaches women's and men's suit.
  `Brust` alone is women's bust, `Brustumfang` both.
- **`top`** stays the Tongan paʻanga and **`kid`** the Kiribati dollar without
  another size word (SPEC_SIZES §2.4).

### 2.4 Countries, adjectives and fillers

**Countries and adjectives as systems** (decided 2026-10-23), only beside a size
word (an item, body, person or `size` word, or a size label): `dress 44 italian
to american`, `vestito 44 italiano`, `italy 44 dress usa`. Without a size word
they keep today's meaning (`12 USA Europe` is a currency).

| System | en                                   | es                    | pt                   | fr                    | de                       |
|--------|--------------------------------------|-----------------------|----------------------|-----------------------|--------------------------|
| `us`   | american, us, usa, america           | americano, estadounidense | americano        | américain             | amerikanisch             |
| `uk`   | british, english, uk, britain        | británico, inglés     | britânico, inglês    | britannique, anglais  | britisch, englisch       |
| `eu`   | european, europe                     | europeo               | europeu              | européen              | europäisch               |
| `it`   | italian, italy                       | italiano              | italiano             | italien               | italienisch              |
| `fr`   | french, france                       | francés               | francês              | français              | französisch              |
| `de`   | german, germany                      | alemán                | alemão               | allemand              | deutsch                  |
| `jp`   | japanese, japan                      | japonés               | japonês              | japonais              | japanisch                |
| `cn`   | chinese, china                       | chino                 | chinês               | chinois               | chinesisch               |
| `int`  | international, letters, letter       | internacional, letras | internacional, letras | international, lettres | international, Buchstaben |

| System | id                  | tr                 | vi           | it                       | pl                    |
|--------|---------------------|--------------------|--------------|--------------------------|-----------------------|
| `us`   | amerika             | amerikan           | mỹ           | americano, americana     | amerykański           |
| `uk`   | inggris             | ingiliz            | anh          | inglese, britannico      | brytyjski, angielski  |
| `eu`   | eropa               | avrupa             | châu âu      | europeo, europea         | europejski            |
| `it`   | italia              | italyan            | ý            | italiano, italiana       | włoski                |
| `fr`   | prancis             | fransız            | pháp         | francese                 | francuski             |
| `de`   | jerman              | alman              | đức          | tedesco, tedesca         | niemiecki             |
| `jp`   | jepang              | japon              | nhật         | giapponese               | japoński              |
| `cn`   | cina, tiongkok      | çin                | trung quốc   | cinese                   | chiński               |
| `int`  | internasional, huruf | uluslararası, harf | quốc tế, chữ | internazionale, lettere | międzynarodowy, litery |

Country names are also read in every language the app's country names already
cover (`time::tz::country`), and the feminine and plural forms of the
adjectives above (`italiana`, `italiane`, `américaine`) as the data file lists
them.

**Size fillers** (decided 2026-10-23) are ignored inside a size request, in any
place: `size 44 it dress in us`, `my size is 44 it`, `what is 44 it in us`.

| en | size, sizes, my size is, what is, what's, convert, in size, size in |
|----|----------------------------------------------------------------------|
| es | talla, mi talla es, qué es, convertir, en talla                     |
| pt | tamanho, meu tamanho é, o que é, converter                          |
| fr | taille, ma taille est, quelle est, convertir, en taille             |
| de | Größe, meine Größe ist, was ist, umrechnen, in Größe                |
| id | ukuran, ukuran saya, berapa, konversi                               |
| tr | beden, numara, bedenim, nedir, çevir                                |
| vi | cỡ, kích cỡ, size, là gì, đổi                                       |
| it | taglia, misura, numero, la mia taglia è, quanto è, converti         |
| pl | rozmiar, mój rozmiar to, ile to, przelicz                           |

- **Punctuation** inside a size request is ignored: `,`, `?`, `!`, `:`
  (`my size is 44 it, what is it in us?`).
- **`size` and its translations alone make a size request** (decided
  2026-10-23): `size 42 eu us` asks for shoes and clothes, no currency (§3.2).
- `numero` and `numara` are the shoe-size words in Italian and Turkish: they
  make a size request as `size` does.
- `Größe` is also SPEC_SIZES §2.3's German height word (kids, phase 3): until
  then it is only a filler.

## 3. Request grammar

### 3.1 What makes a clothes request

A request is a **clothes request** when it has (decided 2026-10-23):

- a **clothes word** anywhere: `clothes`, an item word (`dress`, `vestito`,
  `suit`), or a clothes body word (`bust`, `chest`, `waist`, `hip`, `collar`);
- and a **size**: a label of §2.2, or a number with `cm`/`in` and a body word.

Any order, any connector (`to`, `in`, `->`, `→`, `a`, `en`, `nach`, as units
accept), the fillers anywhere:

- `44 it dress us`, `dress 44 it to us`, `IT 44 dress US`, `dress IT44 → US`
- `vestito 44 italiano in americano`, `my dress size is 44 italian, what is it
  in us`
- `clothes M it`, `M women top it`, `top m int to it`
- `W32 trousers eu`, `32x34 pants it`, `trousers 32 waist eu`
- `92 cm bust`, `bust 92 cm us`, `36 in chest eu`, `39 cm collar us`

### 3.2 The person word, and `size`

- **A person word with no item** (`12 us woman eu`, `42 eu women us`) means
  **clothes first, then shoes** (decided 2026-10-23): the clothes answer is the
  result, the shoes line follows. This replaces SPEC_SIZES §3.2's "a person word
  alone means clothing, shoes need the shoe word".
  - `42 eu women us` → `US 12`, then `shoes: 42 eu = US 10 women`.
- **`size` alone** (`size 42 eu us`, `taglia 44 it us`) means **sizes, shoes
  first**: the shoes answer is the result, the clothes line follows. No
  currency.

### 3.3 No size word: the additional results

Without a size word, a request is read as today (currency first, then units,
dates…), and **every size reading that matches the whole input** is added as a
line, as the shoes line beside a currency is today (SPEC_SIZES §4.1):

```text
$ aequalis 42 eu us
- 42 EUR = 47.15 USD
- shoes: 42 eu = US 8.5 men, US 10 women
- clothes: 42 eu = US 12 women, US 26 men's trousers, US 16.5 men's shirt
```

- **When:** every word fits a clothes request (a size label and systems, the
  connectors), and the size is in at least one chart's range. A word that is no
  size word (`kg`, a city) means no line.
- **The order** (decided 2026-10-23): the request's own answer (currency, unit),
  then the shoes line, then the clothes line.
- **`it`, `fr`, `de`, `jp`, `cn`, `int`** are no currency codes. A request with
  them has no currency answer: see §3.4's "only sizes answer".
- **No target** (`42 eu`): the currency's answer or hint, then the lines in the
  region's system (§4.2); a line whose sizes are all in the typed system is
  left out, as the shoes line is.

### 3.4 Several readings: all of them, by priority

A request that means several things answers **every reading that matches, in
priority order**, never a question (decided 2026-10-23). This replaces
SPEC_SIZES §5's `which?` and `which sizing?` questions.

1. **The explicit word wins** the result: a clothes word makes the clothes
   answer the result, `shoes` the shoes answer, `size` the shoes answer, a
   currency or unit request without a size word its own answer.
2. **The other readings follow**, each as one line: shoes before clothes.
3. **Inside clothes**, the charts in §1's order: women's, men's suit, men's
   trousers, men's shirt. A person or item word narrows them (`men` leaves the
   three men's charts, `dress` the women's).
4. **Only sizes answer** (decided 2026-10-23): when nothing else reads the
   request (`44 it us`, `M it`, `W32 eu`), the size readings become the result,
   shoes first, instead of today's error:

   ```text
   $ aequalis 44 it us
   - shoes: 44 eu = US 10 men, US 11.5 women
   - clothes: 44 it = US 8 women, US 34 men's suit, US 28 men's trousers
   ```

### 3.5 Clashes

- **Letters and units:** `M`, `S`, `L`, `XL` are sizes only in a size request
  or when the whole input reads as one (`M it`, `clothes M`). `10 m` stays
  10 metres, `3 s` 3 seconds, `2 l` 2 litres: a number before a letter is no
  size label.
- **`cm` and `in`** with a body word are a measure (`39 cm collar`,
  `36 in chest`); without one, a length (`39 cm` → inches, as today).
- **`in`** is a connector after an item or size (`dress 44 it in us`), inches
  after a number with a body word (`15.5 in neck`), and inches with no size word
  (`42 in` is a length).
- **`it`, `de`** are systems next to a size (`it 44`, `44 it`). Elsewhere in a
  size request they are words: the English pronoun `it` (`what is it in us`)
  and the Spanish and Portuguese `de` are fillers there.
- **CN labels** (`160/84A`) are sizes only with a clothes word or `cn`;
  `160/84` alone stays a division.
- **Waist labels** `32x34` and `32/34` are sizes only with a trousers or waist
  word; alone they stay arithmetic.
- **`top`, `kid`** keep SPEC_SIZES §2.4's rule.

### 3.6 A size with no system

As SPEC_SIZES §3.3 for shoes (decided 2026-10-23): a size typed without its
system (`vestito 44`) is read in the **region's system** (§4.2) when that system
has it; else in the **only system** that has it; else the hint asks which,
listing the systems that have it. `vestito 44` in Rome is IT 44; in New York
(US sizes stop at 18) it asks `which system: de, fr, it`. A letter needs no
system (`dress M`).

### 3.7 Partial requests (as you type)

| Typed            | Hint                                                     |
|------------------|----------------------------------------------------------|
| `clothes`        | `now the size: 44 it, M, 8 us…`                          |
| `clothes 44`     | the systems that have a 44: `which system: de, fr, it`   |
| `dress 44 it`    | the answer in the region's system, the others as details |
| `clothes 44 it`  | a line per chart (§4.1)                                  |
| `92 cm`          | the length, as today                                     |
| `92 cm bust`     | the answer in the region's system                        |

Every prefix of a valid request gives a hint or an answer, never a hard error
(the fuzzer checks it, §8).

## 4. Answers

### 4.1 Shape

- **One chart:** the target size, `US 8`; the other systems as the details
  line, `UK 12 · DE 38 · FR 40 · IT 44 · JP 11 · M · CN 165/92A`; the notes
  (the chart, `women's clothes`; `EU sizes as in DE`; "between"; the brand
  note).
- **Several charts:** a line per chart, in priority order, each named:

  ```text
  $ aequalis clothes M it
  IT 44 women
  IT 48 men's suit
  IT 48–49 men's trousers
  ```

  The first line is the result (`--json` `result`, Copy's value); the others are
  `extra`, `{"type":"clothes men's suit","results":["IT 48"]}`.
- **No target, no person** (`clothes 44 it`): a line per chart, each with its
  own systems (decided 2026-10-23):

  ```text
  $ aequalis clothes 44 it
  women           US 8 · UK 12 · DE 38 · FR 40 · IT 44 · JP 11 · M · CN 165/92A
  men's suit      US 34 · EU 44 · JP S · XS
  men's trousers  US 28 · EU 44 · XS
  ```

- **No target, one chart:** the region's system as the result, the others as
  the details (§4.2).
- **The clothes line** (§3.3) is one line naming every chart (decided
  2026-10-23): `clothes: 42 eu = US 12 women, US 26 men's trousers, US 16.5
  men's shirt`. No notes: the line is a summary.
- **Copy** as for shoes (SPEC_SIZES §4.1): one chart, its value (`8`); several,
  every line (`Women IT 44` / `Men's suit IT 48`); beside a currency, the
  currency's.
- **Errors:** a size no chart has, `no IT 70 in clothes (IT 36–54 women,
  IT 44–60 men's suit, IT 42–56 men's trousers)`; kids,
  `kids' clothes: not yet`.

### 4.2 No target: the region

The region is the device's time zone's country (SPEC_SIZES §4.2). Clothes use
its Women's and Men's clothing columns:

| Country                 | Women's | Men's suit | Men's trousers | Men's shirt |
|-------------------------|---------|------------|----------------|-------------|
| US, LR, MM              | US      | US         | US             | US (in)     |
| UK, IE                  | UK      | UK         | UK             | UK (in)     |
| Italy                   | IT      | IT         | IT             | EU (cm)     |
| FR, BE, ES, PT          | FR      | EU         | EU             | EU (cm)     |
| DE, AT, CH, NL, Nordics | DE      | EU         | EU             | EU (cm)     |
| Japan                   | JP      | JP         | EU             | EU (cm)     |
| China                   | CN      | EU         | EU             | EU (cm)     |
| anywhere else           | EU (DE) | EU         | EU             | EU (cm)     |

The men's charts have no CN column, so China reads EU for them (decided
2026-10-23, in place of SPEC_SIZES §4.2's CN for men's clothing).

### 4.3 Body measures

- A measure takes the **nearest row** of each chart that uses it, with
  SPEC_SIZES §4.4's "between" note and its "beyond the chart" note.
- **Inches** are × 2.54; `in` or `"` after the number.
- **Several charts** (a waist): a line each, in priority order:
  `81 cm waist` → `UK 14 women`, `UK 32 men's trousers` (in Dublin).
- **Women's bust, waist and hip** each read the women's chart on their own
  column; two measures in one request (`92 bust 75 waist`) are not supported in
  this version (an error, `one measure at a time`).

### 4.4 Ranges and repeated labels

- **A range** answers a range: `8-10 us dress it` → `IT 44–46`;
  `S-M women top it` → `IT 40–44`.
- **A label on two rows** (women's `XXL`, men's JP `S`) answers both, as a
  range: `XXL dress us` → `US 16–18`. As SPEC_SIZES §4.4's "value twice in a
  column".
- **Between rows** and **at the midpoint**: SPEC_SIZES §4.4.

## 5. Ambiguity

There are no questions for clothes (§3.4). The table replaces SPEC_SIZES §5 for
clothing:

| Situation                               | Answer                                    |
|-----------------------------------------|-------------------------------------------|
| No person, several charts               | a line per chart, in priority order       |
| `man` or `clothes` for men, no item     | the three men's charts, a line each       |
| Shirt: by collar or by letters          | collar first when a number, letters when a letter |
| A size in no chart                      | error with every chart's range            |
| `eu` for women's                        | DE, note `EU sizes as in DE`              |

## 6. Charts

### 6.1 Women's: dresses, tops and trousers

Sizes: the offsets most retail tables agree on
([size.ly](https://www.size.ly/conversion-charts/international-size-charts-women),
[ASOS](https://www.asos.com/discover/size-charts/women/dresses/)), as SPEC_SIZES
§6.2 decided: UK = US + 4, DE = US + 30, FR = US + 32, IT = US + 36,
JP = US + 3; `IT 44 = FR 40 = DE 38 = UK 12 = US 8`. CN: GB/T 1335 labels.

Body measures (cm): ASOS's women's chart, UK 4–18
([size.ly's copy of the ASOS guide](https://www.size.ly/size-chart/asos)), and
ASOS Curve for UK 20–22
([ASOS DE](https://www.asos.com/de/entdecken/grossentabelle/damen)).

| US | UK | DE/EU | FR | IT | JP | INT  | CN       | Bust  | Waist | Hip   |
|----|----|-------|----|----|----|------|----------|-------|-------|-------|
| 0  | 4  | 30    | 32 | 36 | 3  | XXS  | 150/76A  | 78    | 60    | 83.5  |
| 2  | 6  | 32    | 34 | 38 | 5  | XS   | 155/80A  | 80.5  | 62.5  | 86    |
| 4  | 8  | 34    | 36 | 40 | 7  | S    | 155/84A  | 83    | 65    | 88.5  |
| 6  | 10 | 36    | 38 | 42 | 9  | S/M  | 160/88A  | 88    | 70    | 93.5  |
| 8  | 12 | 38    | 40 | 44 | 11 | M    | 165/92A  | 93    | 75    | 98.5  |
| 10 | 14 | 40    | 42 | 46 | 13 | L    | 165/96A  | 98    | 80    | 103.5 |
| 12 | 16 | 42    | 44 | 48 | 15 | L/XL | 170/100A | 103   | 85    | 108.5 |
| 14 | 18 | 44    | 46 | 50 | 17 | XL   | 170/104A | 110.5 | 92.5  | 116   |
| 16 | 20 | 46    | 48 | 52 | 19 | XXL  | 175/110A | 116   | 98    | 123.5 |
| 18 | 22 | 48    | 50 | 54 | 21 | XXL  | 175/116A | 123   | 105   | 130.5 |

ASOS's "EU" column is the FR number (UK 8 = EU 36); Aequalis's `eu` is DE
(§2.1), and FR has its own system.

### 6.2 Men's suits and jackets

Sizes: chest in inches (US = UK), EU/IT/FR = US + 10, JP letters (SPEC_SIZES
§6.3). Letters: ASOS's men's chest ranges (XS 34–36 in, S 36–38, M 38–40,
L 40–42, XL 42–44, XXL 44–46, XXXL 46–48;
[ASOS men's suits](https://www.asos.com/discover/size-charts/men/suits-tailoring/),
[size.ly's copy](https://www.size.ly/size-chart/asos)), each row the letter
whose range starts at its chest (decided 2026-10-23: in place of SPEC_SIZES §6.3's
letters, which put 42 at L and 44 at XL). The chest measure is the size itself:
`100 cm chest` is 39.4 in.

| US/UK (in) | EU/IT/FR | JP  | INT  | Chest (cm) |
|------------|----------|-----|------|------------|
| 34         | 44       | S   | XS   | 86         |
| 36         | 46       | S   | S    | 91         |
| 38         | 48       | M   | M    | 97         |
| 40         | 50       | L   | L    | 102        |
| 42         | 52       | L   | XL   | 107        |
| 44         | 54       | LL  | XXL  | 112        |
| 46         | 56       | LL  | XXXL | 117        |
| 48         | 58       | 3L  |      | 122        |
| 50         | 60       | 3L  |      | 127        |

**The men's letters** (`clothes M`, `top m`, `shirt XL`) are this chart's INT
column.

### 6.3 Men's trousers

Sizes and waist: ASOS's men's trousers chart
([size.ly's copy](https://www.size.ly/size-chart/asos)), the waist in inches
(US = UK = W), EU/IT/FR/DE the continental number. Letters: ASOS's men's waist
ranges (XXS 26–28 in, XS 28–30, S 30–32, M 32–34, L 34–36, XL 36–38, XXL 38–40,
XXXL 40–42;
[ASOS men's underwear chart](https://www.asos.com/us/discover/size-charts/men/underwear-loungewear-socks/)).

| W (US/UK) | EU/IT | INT  | Waist (cm) |
|-----------|-------|------|------------|
| 26        | 42    | XXS  | 66         |
| 28        | 44    | XS   | 71         |
| 29        | 45    | XS   | 73.5       |
| 30        | 46    | S    | 76         |
| 31        | 47    | S    | 78.5       |
| 32        | 48    | M    | 81         |
| 33        | 49    | M    | 83.5       |
| 34        | 50    | L    | 86         |
| 36        | 52    | XL   | 91         |
| 38        | 54    | XXL  | 96         |
| 40        | 56    | XXXL | 101        |

A letter on several rows (`M`: W32 and W33) answers both, as a range
(`W32–33`).

### 6.4 Men's shirts

Collar in cm (EU) and inches (US = UK), as SPEC_SIZES §6.3; letters as German
shirt makers write them (M = 39/40, L = 41/42;
[hemden.de](https://www.hemden.de/en/converting-shirt-sizes)); the inches as
[Premiere Manche](https://www.premieremanche.com/en/content/22-shirt-size-guide)
pairs them, except 40 cm, which is 15.75 in (40 ÷ 2.54; Premiere Manche rounds
it to 15.5, which is also 39's).

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

## 7. UI

One **Sizes** feature for shoes and clothes (decided 2026-10-23), in the TUI,
on Android and in the Omarchy widget.

- **Group keys: Shoes and Clothes.** Shoes writes `shoes`, Clothes writes
  `clothes`; each replaces the other's word when there (`shoes 42 eu` + Clothes
  → `clothes 42 eu`). Item words (`dress`, `suit`) are typed or come from the
  suggestions.
- **People keys** Men, Women, Kids, as today (SPEC_SIZES §7).
- **System keys** EU, UK, US, IT, FR, DE, JP, CN, and **INT**.
- **Letter keys** XS, S, M, L, XL, XXL (decided 2026-10-23), after the systems,
  in Android's Sizes keys, strip and panel, and the TUI's Sizes window and
  board.
- **A system key writes only its code** (decided 2026-10-23): `42` + EU →
  `42 eu`. With nothing specified the answer brings everything: the currency,
  the shoes line, the clothes line. This replaces SPEC_SIZES §7's "a system key
  brings `shoes`".
- **Every line is drawn**, as the shoes list beside a currency is today: the
  TUI's answer takes the rows it needs (live and in the history); Android grows
  the result area, a line per reading, in the shoes table's style
  (`Women  IT 44`); the widget lists the `extra` lines.
- **Measures panel:** the Sizes class gets the Clothes group beside Shoes.
- **Trivia: a Clothes section** (decided 2026-10-23), beside Shoes, with four
  entries, Women's clothes, Men's suits, Men's trousers and Men's shirts, each a
  table of §6 with its body measures (cm); a row writes its request
  (`clothes 44 it women`).
- **Suggestions while typing:** the item, body, filler and country words of
  the 10 languages, and the systems and letters after a size in a size request.
- **Labels are terse**, as everywhere: `IT 44 women`, `men's suit`.

## 8. Tests and fuzz

### 8.1 Tests

`tests/sizes.rs` on the simulated `italy` and `us` machines, as phase 1. **Each
case of §9 is written in every phrasing that should give the same answer**
(decided 2026-10-23): connectors (none, `to`, `in`, `->`, `→`, the other
languages'), word order, case, accents, attached and spaced systems, fillers,
countries and adjectives, the item word in each of the 10 languages. A helper
expands one case into its phrasings, so the matrix grows with the vocabulary.
Unit tests in `src/sizes.rs` cover the parser pieces: labels, ranges, CN
labels, waist labels, letters, the clashes of §3.5.

### 8.2 Fuzz

`fuzz/generate.py` makes shoe and clothes requests (every label kind, item,
body, person, filler and country word, connectors, systems, partial prefixes,
and currencies that could be sizes). `fuzz/worker.py` and `fuzz/oracles.py`
check, beside no crash and no hang (decided 2026-10-23):

- **Same answer, any phrasing:** connectors, order, case, accents, attached vs
  spaced systems, fillers, translations of the item word.
- **Round trip:** a size converted to another system and back reads the same
  row (`IT 44` → `US 8` → `IT 44`), for every chart and system pair.
- **Extra lines consistent:** a size line beside a currency (or beside another
  size answer) equals the explicit request's answer (`42 eu us`'s clothes line
  = `clothes 42 eu us`).
- **Partial typing:** every prefix of a valid size request gives a hint or an
  answer, never a hard error.

## 9. Test cases

Machine `any` means the same on `italy` and `us`. Each row is also run in its
other phrasings (§8.1).

| Request                          | Machine | Expected                                                          |
|----------------------------------|---------|-------------------------------------------------------------------|
| `44 it dress us`                 | any     | `US 8`; details `UK 12 · DE 38 · FR 40 · JP 11 · M · CN 165/92A`  |
| `IT44 dress US`                  | any     | `US 8`                                                            |
| `dress 44 italian to american`   | any     | `US 8`                                                            |
| `my dress size is 44 it, what is it in us` | any | `US 8`                                                     |
| `vestito 44`                     | italy   | `IT 44`, details the others                                       |
| `vestito 44`                     | us      | hint `which system: de, fr, it` (§3.6)                            |
| `us 8 dress`                     | italy   | `IT 44`                                                           |
| `M women top it`                 | any     | `IT 44`                                                           |
| `clothes M it`                   | any     | `IT 44 women` / `IT 48 men's suit` / `IT 48–49 men's trousers`    |
| `12 us woman eu`                 | any     | `EU 42`, note `EU sizes as in DE`; `shoes: 12 us = EU 44.5 women` |
| `42 eu women us`                 | any     | `US 12`; `shoes: 42 eu = US 10 women`                             |
| `40 us man eu`                   | any     | `EU 50 men's suit` / `EU 56 men's trousers`                       |
| `40 us suit eu`                  | any     | `EU 50`                                                           |
| `40 us suit int`                 | any     | `L`                                                               |
| `39 cm collar`                   | us      | `US 15.5`                                                         |
| `15.5 in neck eu`                | any     | `EU 39`                                                           |
| `160/92A dress us`               | any     | `US 8`                                                            |
| `XXL dress us`                   | any     | `US 16–18`                                                        |
| `8-10 us dress it`               | any     | `IT 44–46`                                                        |
| `W32 L34 trousers eu`            | any     | `EU 48`, note `inseam L34 has no EU size`                         |
| `32x34 pants it`                 | any     | `IT 48`, the inseam note                                          |
| `92 cm bust`                     | us      | `US 8`                                                            |
| `100 cm chest`                   | italy   | `IT 50`, note `between US 38 and 40`                              |
| `81 cm waist`                    | us      | `US 10 women` / `US 32 men's trousers`                            |
| `clothes 44 it`                  | any     | a line per chart (§4.1)                                           |
| `size 42 eu us`                  | any     | shoes result `US 8.5 men, US 10 women`; the clothes line          |
| `42 eu us`                       | any     | the currency; the shoes line; the clothes line (§3.3)             |
| `44 it us`                       | any     | the shoes line, then the clothes line, as the result (§3.4)       |
| `M it`                           | any     | `clothes: M = IT 44 women, IT 48 men's suit, IT 48–49 men's trousers` |
| `10 m`                           | any     | 10 metres' answer, no size line                                   |
| `160/84`                         | any     | the division, no size line                                        |
| `clothes 128 kids`               | any     | error `kids' clothes: not yet`                                    |
| `clothes 70 it`                  | any     | error with every chart's range (§4.1)                             |
| `clothes`                        | any     | hint `now the size: 44 it, M, 8 us…`                              |

## 10. Decisions (2026-10-23)

1. Clothes follow the shoes' strategy: a clothes word makes a clothes request;
   without one, matching readings are extra lines.
2. Any clothes word counts, anywhere: `clothes`, an item word, a clothes body
   word.
3. Several readings: all of them, by priority; no questions (replaces
   SPEC_SIZES §5's `which?` and `which sizing?`).
4. The explicit word wins the result; the others follow, shoes before clothes.
5. A person word alone: clothes first, then the shoes line (replaces SPEC_SIZES
   §3.2's rule).
6. `size` and its translations make a size request: shoes first, then clothes.
7. When only sizes answer, they are the result instead of an error.
8. Charts: women's, men's suit, men's trousers, men's shirt, in that priority;
   the last two are technical and come last.
9. Common tables, no brand charts; the brand note on every answer.
10. Body measures for every chart, from ASOS's EN 13402 measures; the measure
    word decides the charts; one measure at a time.
11. Kids' clothes stay in phase 3: `kids' clothes: not yet`.
12. Accepted: attached systems, countries and adjectives (only beside a size
    word), fillers, letters, ranges, CN labels, waist labels.
13. Natural phrasing for every request is a separate spec, after clothes, in
    the 10 languages (TODO `nl-1`).
14. No target, no person: a line per chart, each with its own systems.
15. Line order: the request's own answer, shoes, clothes; the clothes line is
    one line naming every chart.
16. A range answers a range; the inseam is ignored with a note.
17. Men's suit letters follow ASOS's chest ranges (42 = XL, 44 = XXL), in place
    of SPEC_SIZES §6.3's.
18. China reads EU for the men's charts (they have no CN column).
19. UI: one Sizes feature; Shoes and Clothes group keys, people, systems with
    INT, letter keys; a system key writes only its code; Trivia gets a Clothes
    section.
20. Tests: every case in every phrasing; fuzz: any phrasing, round trip, extra
    lines, partial typing.
21. The spec is `docs/SPEC_CLOTHES.md`.

## 11. Phases

One release, a `minor`, when all of it works (nothing ships without the TUI and
Android):

1. **Core:** the charts and words in `src/data/sizes.json` (compiled by
   `build/tables.rs`), the rules in `src/sizes.rs`, the readings' order in
   `src/lib.rs`, the tests (§8.1).
2. **Fuzz:** shoes and clothes requests and the oracles (§8.2).
3. **TUI:** the Sizes window and board, the lines, Trivia.
4. **Android:** the Sizes keys, strip and panel, the lines, Trivia.
5. **Widget:** the extra lines.
6. **Tried on the phone**, then released as a `minor`.
