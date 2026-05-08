# GPT Image 2 Prompt: PID 文件格式信息图

Use case: infographic-diagram
Asset type: technical documentation poster / README visual
Primary request: Generate a high-resolution technical infographic explaining the SmartPlant / Smart P&ID `.pid` file format and related offline publish pipeline.

Canvas:
- Aspect ratio: 16:9 landscape, 3840x2160 preferred
- Background: deep slate engineering grid, subtle 1px grid lines, clean technical diagram style
- Layout: left-to-right layered architecture with one lower pipeline band
- Typography: crisp monospaced technical labels; all visible text must be legible, exact, and not garbled
- Visual style: modern engineering documentation, restrained colors, no cartoon style, no 3D, no decorative blobs

Title text, verbatim:
"SmartPlant P&ID .pid File Format"

Subtitle text, verbatim:
"Layered CFB container, metadata streams, object storages, clusters, dynamic attributes, and publish XML pipeline"

Main diagram content:
1. Left section: `.pid` file as an OLE / CFBF compound file container.
   - Draw a file icon labeled `.pid`
   - Draw nested storage tree boxes:
     - `Root Entry`
     - `SummaryInformation`
     - `DocumentSummaryInformation`
     - `TaggedTxtData / Drawing`
     - `TaggedTxtData / General`
     - `JSite0 ... JSiteN`
     - `PSMcluster0`
     - `StyleCluster`
     - `Dynamic Attributes Metadata`
     - `Unclustered Dynamic Attributes`
     - `Sheet0 ... SheetN`
     - `PSMroots`

2. Center section: parser interpretation layers.
   - Five stacked horizontal bands labeled:
     - `Layer 1: OLE / CFBF Container`
     - `Layer 2: Tagged Metadata`
     - `Layer 3: JSite Objects`
     - `Layer 4: Clusters and Sheets`
     - `Layer 5: Dynamic Attributes`
   - Add small callouts:
     - `stream path + size + preview`
     - `DrawingMeta / GeneralMeta`
     - `.sym references + OLE links`
     - `cluster index + sheet records`
     - `ASCII / UTF-16LE strings + relationships`

3. Right section: enriched parser output.
   - Draw a structured object graph box labeled `PidDocument`
   - Inside it show:
     - `SummaryInfo`
     - `DrawingMeta`
     - `JSite[]`
     - `ClusterInfo[]`
     - `DynamicAttributeBlob`
     - `Object Graph`
     - `Cross Reference Graph`
     - `Layout Model`
   - Draw arrows from the five parser layers into `PidDocument`.

4. Bottom band: offline publish XML pipeline.
   - Use separate pipeline boxes connected by arrows:
     - `Export.dmp`
     - `MTF envelope strip`
     - `Export.mdf`
     - `oxidized-mdf`
     - `SQLite staging`
     - `PublishDrawing DTO`
     - `_Data.xml`
     - `_Meta.xml`
   - Add table chips around MDF stage:
     - `T_Drawing`
     - `T_ModelItem`
     - `T_Representation`
     - `T_Relationship`
     - `T_PipingPoint`
     - `attributes`
     - `codelists`

Color system:
- CFB container: cyan / blue
- Metadata: violet
- JSite objects: emerald
- Clusters and sheets: amber
- Dynamic attributes: rose
- Publish pipeline: neutral slate with orange MDF accent

Constraints:
- Must be an accurate technical infographic, not a decorative poster.
- Must show `.pid` as a layered OLE / CFBF compound container.
- Must show the publish pipeline as separate from direct `.pid` parsing.
- Must include exact labels listed above.
- Avoid fake code, fake numeric metrics, random UI panels, unreadable tiny paragraphs, watermarks, logos, or invented product branding.
- Keep arrows orthogonal and labels separated from lines.
- Use consistent spacing, alignment, and hierarchy.
