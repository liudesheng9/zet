# Dot-sourced by the book-digest scripts: a small TF-IDF over Chinese character bigrams (plus
# Latin words and section numbers), used to tell each DeepSeek run which other sections and Cards
# are most likely related to its own, so it reads less to find them.

function Get-Terms([string]$text) {
    $terms = @{}
    $clean = [regex]::Replace($text, '\$[^$]*\$|\[\[[^\]]*\]\]|`[^`]*`', ' ')
    foreach ($run in [regex]::Matches($clean, '[㐀-鿿]+')) {
        $s = $run.Value
        for ($i = 0; $i + 1 -lt $s.Length; $i++) {
            $bigram = $s.Substring($i, 2)
            $terms[$bigram] = 1 + [int]$terms[$bigram]
        }
    }
    foreach ($word in [regex]::Matches($clean.ToLowerInvariant(), '[a-z][a-z\-]{3,}')) {
        $terms[$word.Value] = 1 + [int]$terms[$word.Value]
    }
    $terms
}

# Build weighted vectors for a list of documents ({ id; text }) and return a scorer.
function New-Relatedness($documents, [int]$keep = 150) {
    $df = @{}
    $raw = @{}
    foreach ($doc in $documents) {
        $terms = Get-Terms $doc.text
        $raw[$doc.id] = $terms
        foreach ($term in $terms.Keys) { $df[$term] = 1 + [int]$df[$term] }
    }
    $count = [Math]::Max(1, $documents.Count)
    $vectors = @{}
    $postings = @{}
    foreach ($id in $raw.Keys) {
        $weights = @{}
        foreach ($term in $raw[$id].Keys) {
            if ($df[$term] -ge $count) { continue }
            $weights[$term] = (1 + [Math]::Log($raw[$id][$term])) * [Math]::Log($count / $df[$term])
        }
        $top = $weights.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First $keep
        $norm = [Math]::Sqrt((($top | ForEach-Object { $_.Value * $_.Value }) | Measure-Object -Sum).Sum)
        if (-not $norm) { $norm = 1 }
        $vector = @{}
        foreach ($entry in $top) {
            $vector[$entry.Key] = $entry.Value / $norm
            if (-not $postings.ContainsKey($entry.Key)) { $postings[$entry.Key] = [Collections.Generic.List[string]]::new() }
            $postings[$entry.Key].Add($id)
        }
        $vectors[$id] = $vector
    }
    [pscustomobject]@{ Vectors = $vectors; Postings = $postings }
}

# The scoring loop is compiled: in script it takes hours on a textbook's thousands of Cards.
if (-not ('BookDigestRelatedness' -as [type])) {
    Add-Type -TypeDefinition @'
using System.Collections;
using System.Collections.Generic;
using System.Linq;
public static class BookDigestRelatedness {
    // Every other id that shares a term with id, most similar first.
    public static string[] Rank(Hashtable vectors, Hashtable postings, string id) {
        var vector = vectors[id] as Hashtable;
        if (vector == null) return new string[0];
        var scores = new Dictionary<string, double>();
        foreach (DictionaryEntry entry in vector) {
            var list = postings[entry.Key] as IEnumerable;
            if (list == null) continue;
            double weight = (double)entry.Value;
            foreach (string other in list) {
                if (other == id) continue;
                double score;
                scores.TryGetValue(other, out score);
                scores[other] = score + weight * (double)((Hashtable)vectors[other])[entry.Key];
            }
        }
        return scores.OrderByDescending(kv => kv.Value).Select(kv => kv.Key).ToArray();
    }
}
'@
}

# Ids most similar to $id, excluding ids for which $exclude returns $true.
function Find-Related($model, [string]$id, [int]$top = 5, [scriptblock]$exclude = { $false }) {
    $found = [Collections.Generic.List[string]]::new()
    foreach ($other in [BookDigestRelatedness]::Rank($model.Vectors, $model.Postings, $id)) {
        if (& $exclude $other) { continue }
        $found.Add($other)
        if ($found.Count -ge $top) { break }
    }
    @($found)
}
