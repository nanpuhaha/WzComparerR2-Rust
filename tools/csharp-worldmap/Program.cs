using System.Text.Json;
using System.Security.Cryptography;
using WzComparerR2.WzLib;
using WzComparerR2.Common;

class Program
{
    static Wz_Node N(string name, object value = null)
    {
        var n = new Wz_Node(name);
        n.Value = value;
        return n;
    }
    static Wz_Node Build(JsonElement e)
    {
        var n = N(e.GetProperty("name").GetString());
        if (e.TryGetProperty("value", out var v))
        {
            if (v.TryGetProperty("Str", out var s)) n.Value = s.GetString();
            else if (v.TryGetProperty("Int", out var i)) n.Value = i.GetInt32();
            else if (v.TryGetProperty("Float", out var f)) n.Value = (float)f.GetDouble();
            else if (v.TryGetProperty("Vector", out var vec))
                n.Value = new Wz_Vector(vec[0].GetInt32(), vec[1].GetInt32());
        }
        if (e.TryGetProperty("children", out var children))
            foreach (var c in children.EnumerateArray()) n.Nodes.Add(Build(c));
        return n;
    }
    static int IntNode(Wz_Node n, string name, int def = 0)
    {
        var c = n?.Nodes[name];
        if (c?.Value == null) return def;
        return Convert.ToInt32(c.Value);
    }
    static object LoadQuest(Wz_Node node)
    {
        var open = new List<int>();
        var close = new List<int>();
        for (int i = 0; ; i++)
        {
            var sub = node.Nodes["openMap"]?.Nodes[i.ToString()];
            if (sub == null) break;
            open.Add(Convert.ToInt32(sub.Value));
        }
        for (int i = 0; ; i++)
        {
            var sub = node.Nodes["closeMap"]?.Nodes[i.ToString()];
            if (sub == null) break;
            close.Add(Convert.ToInt32(sub.Value));
        }
        return new {
            name = node.Text,
            open_maps = open,
            close_maps = close,
            is_default = IntNode(node, "default") != 0,
            quest = IntNode(node, "quest"),
            state = IntNode(node, "state"),
        };
    }
    static object LoadFog(Wz_Node node) => new {
        name = node.Text,
        quest = IntNode(node, "quest"),
        state = IntNode(node, "qState"),
    };
    static void Fixture(string path)
    {
        var root = Build(JsonDocument.Parse(File.ReadAllText(path)).RootElement);
        var quests = new List<object>();
        var ql = root.Nodes["QuestLimit"];
        if (ql != null)
        {
            if (ql.Nodes["default"] != null) quests.Add(LoadQuest(ql.Nodes["default"]));
            for (int i = 0; ; i++)
            {
                var q = ql.Nodes[i.ToString()];
                if (q == null) break;
                quests.Add(LoadQuest(q));
            }
        }
        var fogs = new List<object>();
        var fog = root.Nodes["Fog"];
        if (fog != null)
            for (int i = 0; ; i++)
            {
                var f = fog.Nodes[i.ToString()];
                if (f == null) break;
                fogs.Add(LoadFog(f));
            }
        var spots = new List<object>();
        var ml = root.Nodes["MapList"];
        if (ml != null)
        {
            var indices = ml.Nodes.Select(n => int.TryParse(n.Text, out var x) ? x : -1)
                .Where(x => x >= 0).OrderBy(x => x);
            foreach (var i in indices)
            {
                var s = ml.Nodes[i.ToString()];
                var pos = s.Nodes["spot"]?.Value as Wz_Vector;
                var mapNos = new List<int>();
                var mn = s.Nodes["mapNo"];
                if (mn != null)
                    for (int j = 0; ; j++)
                    {
                        var c = mn.Nodes[j.ToString()];
                        if (c == null) break;
                        mapNos.Add(Convert.ToInt32(c.Value));
                    }
                spots.Add(new {
                    x = pos?.X ?? 0, y = pos?.Y ?? 0,
                    type = IntNode(s, "type"), map_nos = mapNos,
                });
            }
        }
        var display = root.Nodes["info"]?.Nodes["WorldMap"]?.Value as string ?? "WorldMapFixture";
        Console.WriteLine(JsonSerializer.Serialize(new { display_name = display, quests, fogs, spots },
            new JsonSerializerOptions { WriteIndented = true }));
    }
    static void Real(string path)
    {
        var st = new Wz_Structure();
        st.Load(path);
        int images = 0, quest = 0, fog = 0;
        var stack = new Stack<Wz_Node>();
        stack.Push(st.WzNode);
        while (stack.Count > 0)
        {
            var n = stack.Pop();
            if (n.Value is Wz_Image img)
            {
                images++;
                if (img.TryExtract())
                {
                    var s2 = new Stack<Wz_Node>();
                    s2.Push(img.Node);
                    while (s2.Count > 0)
                    {
                        var c = s2.Pop();
                        foreach (Wz_Node ch in c.Nodes)
                        {
                            if (ch.Text == "QuestLimit") quest++;
                            if (ch.Text == "Fog") fog++;
                            s2.Push(ch);
                        }
                    }
                }
            }
            foreach (Wz_Node ch in n.Nodes) stack.Push(ch);
        }
        Console.WriteLine(JsonSerializer.Serialize(new { images, quest, fog }));
    }
    static int CountNodes(Wz_Node n)
    {
        int count = 1;
        foreach (Wz_Node c in n.Nodes) count += CountNodes(c);
        return count;
    }
    static void MapNodes(string path, string imageName)
    {
        var st = new Wz_Structure();
        st.Load(path);
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        var img = imgNode?.Value as Wz_Image;
        if (img == null) { Console.WriteLine("{\"error\":\"image not found\"}"); return; }
        img.TryExtract();
        Console.WriteLine(JsonSerializer.Serialize(new { image = imageName, nodes = CountNodes(img.Node) }));
    }
    static void DumpPaths(Wz_Node n, string prefix, List<string> paths)
    {
        paths.Add(prefix);
        foreach (Wz_Node c in n.Nodes)
            DumpPaths(c, prefix + "/" + c.Text, paths);
    }
    static void NodePaths(string path, string imageName)
    {
        var st = new Wz_Structure();
        st.Load(path);
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        var img = imgNode?.Value as Wz_Image;
        if (img == null) { Console.WriteLine("{\"error\":\"image not found\"}"); return; }
        img.TryExtract();
        var paths = new List<string>();
        DumpPaths(img.Node, "", paths);
        Console.WriteLine(JsonSerializer.Serialize(paths));
    }
    static string PropLine(string key, long value)
    {
        string sign = value > 0 ? "+" : "";
        return key switch
        {
            "incSTR" => $"STR : {sign}{value}",
            "incDEX" => $"DEX : {sign}{value}",
            "incINT" => $"INT : {sign}{value}",
            "incLUK" => $"LUK : {sign}{value}",
            "incAllStat" or "statR" => $"올스탯 : {sign}{value}",
            "incMHP" => $"최대 HP : {sign}{value}",
            "incMMP" => $"최대 MP : {sign}{value}",
            "incPAD" => $"공격력 : {sign}{value}",
            "incMAD" => $"마력 : {sign}{value}",
            "incPDD" => $"방어력 : {sign}{value}",
            "incMDD" => $"마법방어력 : {sign}{value}",
            "incACC" => $"명중치 : {sign}{value}",
            "incEVA" => $"회피치 : {sign}{value}",
            "incSpeed" => $"이동속도 : {sign}{value}",
            "incJump" => $"점프력 : {sign}{value}",
            "incCraft" => $"손재주 : {sign}{value}",
            "incDAMr" or "damR" => $"데미지 : {sign}{value}%",
            "incCr" => $"크리티컬 확률 : {sign}{value}%",
            "incCDr" => $"크리티컬 데미지 : {sign}{value}%",
            "incBDR" or "bdR" => $"보스 몬스터 공격 시 데미지 +{value}%",
            "incIMDR" or "imdR" => $"몬스터 방어율 무시 : +{value}%",
            "incEXPr" => $"추가 경험치 : {sign}{value}%",
            "tuc" => $"업그레이드 가능 횟수 : {value}",
            "reqLevel" => $"착용 레벨 : {value}",
            "only" when value != 0 => "중복 소지 불가",
            "tradeBlock" when value != 0 => "교환 불가",
            "equipTradeBlock" when value != 0 => "장착 시 교환 불가",
            "accountSharable" when value != 0 => "월드 내 나의 캐릭터 간 이동만 가능",
            "onlyEquip" when value != 0 => "중복 장착 불가",
            "notExtend" when value != 0 => "유효기간 연장 불가",
            "noPotential" when value != 0 => "잠재능력 설정 불가",
            "fixedPotential" when value != 0 => "잠재능력 재설정 불가",
            "superiorEqp" when value != 0 => "아이템 강화 성공시 더욱 높은 효과를 받을 수 있습니다.",
            "blockGoldHammer" when value != 0 => "황금망치 사용 불가",
            "cantRepair" when value != 0 => "수리 불가",
            _ => null,
        };
    }
    static void GearTooltip(string path, string imageName)
    {
        var st = new Wz_Structure();
        st.Load(path);
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        var img = imgNode?.Value as Wz_Image;
        if (img == null) { Console.WriteLine("{\"error\":\"image not found\"}"); return; }
        img.TryExtract();
        var info = img.Node.Nodes["info"] ?? img.Node;
        var order = new[] {
            "reqLevel","incSTR","incDEX","incINT","incLUK","incAllStat","statR",
            "incMHP","incMMP","incPAD","incMAD","incPDD","incMDD","incACC","incEVA",
            "incSpeed","incJump","incCraft","incDAMr","incCr","incCDr","incBDR",
            "incIMDR","incEXPr","tuc","only","tradeBlock","equipTradeBlock",
            "accountSharable","onlyEquip","notExtend","noPotential","fixedPotential",
            "superiorEqp","blockGoldHammer","cantRepair"
        };
        var lines = new List<string>();
        foreach (var key in order)
        {
            var c = info.Nodes[key];
            if (c?.Value == null) continue;
            var line = PropLine(key, Convert.ToInt64(c.Value));
            if (line != null) lines.Add(line);
        }
        Console.WriteLine(JsonSerializer.Serialize(lines, new JsonSerializerOptions { WriteIndented = true }));
    }
    static Wz_Node FindImage(Wz_Structure st, string name)
    {
        var stack = new Stack<Wz_Node>();
        stack.Push(st.WzNode);
        while (stack.Count > 0)
        {
            var n = stack.Pop();
            var img = n.Value as Wz_Image;
            if (img != null && img.Name == name && img.TryExtract()) return img.Node;
            foreach (Wz_Node c in n.Nodes) stack.Push(c);
        }
        return null;
    }
    static void SetTooltip(string path, int setId)
    {
        var st = new Wz_Structure(); st.Load(path);
        var root = FindImage(st, "SetItemInfo.img");
        var set = root?.Nodes[setId.ToString()];
        if (set == null) { Console.WriteLine("{\"error\":\"set not found\"}"); return; }
        var effects = new List<object>();
        var effectRoot = set.Nodes["Effect"];
        if (effectRoot != null)
            foreach (Wz_Node e in effectRoot.Nodes)
            {
                var lines = new List<string>();
                foreach (Wz_Node stat in e.Nodes)
                {
                    if (stat.Value == null) continue;
                    var line = PropLine(stat.Text, Convert.ToInt64(stat.Value));
                    if (line != null) lines.Add(line);
                }
                effects.Add(new { count = Convert.ToInt32(e.Text), lines });
            }
        Console.WriteLine(JsonSerializer.Serialize(new {
            name = set.Nodes["setItemName"]?.Value as string,
            complete = Convert.ToInt32(set.Nodes["completeCount"]?.Value ?? 0),
            effects
        }, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void PotentialTooltip(string path, int code, int level)
    {
        var st = new Wz_Structure(); st.Load(path);
        var root = FindImage(st, "ItemOption.img");
        var option = root?.Nodes[$"{code:D6}"]?.Nodes["level"]?.Nodes[level.ToString()];
        if (option == null) { Console.WriteLine("[]"); return; }
        var lines = new List<string>();
        foreach (Wz_Node stat in option.Nodes)
        {
            if (stat.Value == null) continue;
            var line = PropLine(stat.Text, Convert.ToInt64(stat.Value));
            if (line != null) lines.Add(line);
        }
        Console.WriteLine(JsonSerializer.Serialize(lines));
    }
    static string Db2Value(object v)
    {
        if (v == null) return "";
        if (v is Wz_Png p) return $"{p.Width}x{p.Height} format={(int)p.Format} len={p.DataLength}";
        if (v is Wz_Vector vec) return $"{vec.X},{vec.Y}";
        if (v is Wz_Sound snd) return $"sound len={snd.DataLength} duration={snd.Ms}";
        if (v is Wz_Uol uol) return uol.Uol;
        return Convert.ToString(v);
    }
    static string Db2Type(object v)
    {
        if (v == null) return "Directory";
        return v switch
        {
            string => "String",
            int => "Int",
            short => "Short",
            long => "Long",
            uint => "Uint",
            float => "Float",
            double => "Double",
            bool => "Bool",
            Wz_Png => "Canvas",
            Wz_Vector => "Vector",
            Wz_Uol => "UOL",
            Wz_Sound => "Sound",
            _ => v.GetType().Name,
        };
    }
    static void DumpDb2(Wz_Node n, string parent, List<object> rows)
    {
        var path = parent == "" ? n.Text : parent + "/" + n.Text;
        rows.Add(new { path, name = n.Text, node_type = Db2Type(n.Value), value = Db2Value(n.Value), children = n.Nodes.Count });
        foreach (Wz_Node c in n.Nodes) DumpDb2(c, path, rows);
    }
    static void Db2(string path, string imageName)
    {
        var st = new Wz_Structure(); st.Load(path);
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        var img = imgNode?.Value as Wz_Image;
        if (img == null) { Console.WriteLine("{\"error\":\"image not found\"}"); return; }
        img.TryExtract();
        var rows = new List<object>();
        DumpDb2(img.Node, "", rows);
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static byte[] PngRgba(Wz_Png p, out int width, out int height)
    {
        width = p.Width; height = p.Height;
        var raw = p.GetRawData();
        if (p.Format == Wz_TextureFormat.ARGB8888)
        {
            var rgba = new byte[width * height * 4];
            for (int i = 0; i < width * height; i++)
            {
                rgba[i*4+0] = raw[i*4+2];
                rgba[i*4+1] = raw[i*4+1];
                rgba[i*4+2] = raw[i*4+0];
                rgba[i*4+3] = raw[i*4+3];
            }
            return rgba;
        }
        // ARGB4444: little-endian ushort, low=ggggbbbb, high=aaaarrrr.
        var out4444 = new byte[width * height * 4];
        for (int i = 0; i < width * height; i++)
        {
            int v = raw[i*2] | (raw[i*2+1] << 8);
            int b = (v & 0xF) * 17;
            int g = ((v >> 4) & 0xF) * 17;
            int r = ((v >> 8) & 0xF) * 17;
            int a = ((v >> 12) & 0xF) * 17;
            out4444[i*4+0] = (byte)r;
            out4444[i*4+1] = (byte)g;
            out4444[i*4+2] = (byte)b;
            out4444[i*4+3] = (byte)a;
        }
        return out4444;
    }
    static void DumpIcons(Wz_Node n, string parent, List<object> rows)
    {
        var path = parent == "" ? n.Text : parent + "/" + n.Text;
        if (n.Value is Wz_Png p && p.ActualScale == 1 && (p.Format == Wz_TextureFormat.ARGB4444 || p.Format == Wz_TextureFormat.ARGB8888))
        {
            var rgba = PngRgba(p, out var w, out var h);
            using var sha = SHA256.Create();
            rows.Add(new { path, width = w, height = h, format = (int)p.Format, sha256 = Convert.ToHexString(sha.ComputeHash(rgba)).ToLowerInvariant() });
        }
        foreach (Wz_Node c in n.Nodes) DumpIcons(c, path, rows);
    }
    static void Icons(string path, string imageName)
    {
        var st = new Wz_Structure(); st.Load(path);
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        var img = imgNode?.Value as Wz_Image;
        if (img == null) { Console.WriteLine("{\"error\":\"image not found\"}"); return; }
        img.TryExtract();
        var rows = new List<object>();
        DumpIcons(img.Node, "", rows);
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemGrid(string itemWzPath, string imageName, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(itemWzPath);
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        var img = imgNode?.Value as Wz_Image;
        if (img == null) { Console.WriteLine("{\"error\":\"image not found\"}"); return; }
        img.TryExtract();

        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        if (!linker.Load(str.wz_files[0], null, null))
        {
            Console.WriteLine("{\"error\":\"string linker load failed\"}");
            return;
        }

        var rows = new List<object>();
        foreach (Wz_Node item in img.Node.Nodes)
        {
            int id;
            if (!int.TryParse(item.Text, out id)) continue;
            linker.StringItem.TryGetValue(id, out var sr);
            Wz_Node icon = item.FindNodeByPath("info\\icon");
            rows.Add(new {
                id = item.Text,
                name = string.IsNullOrEmpty(sr?.Name) ? "  " : sr.Name,
                desc = string.IsNullOrEmpty(sr?.Desc) ? "  " : sr.Desc,
                has_icon = icon != null,
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void MapGrid(string mapWzPath, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(mapWzPath);
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        if (!linker.Load(str.wz_files[0], null, null))
        {
            Console.WriteLine("{\"error\":\"string linker load failed\"}");
            return;
        }
        var rows = new List<object>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            var img = n.Value as Wz_Image;
            if (img == null) continue;
            var id = n.Text.Replace(".img", "");
            if (id.Length == 0 || !id.All(char.IsDigit)) continue;
            int idNum;
            if (!int.TryParse(id, out idNum)) continue;
            linker.StringMap.TryGetValue(idNum, out var sr);
            img.TryExtract();
            var linkNode = img.Node.FindNodeByPath(@"info\link");
            int? link = linkNode?.Value == null ? (int?)null : Convert.ToInt32(linkNode.Value);
            rows.Add(new {
                id,
                street_name = sr?.StreetName ?? "",
                map_name = sr?.MapName ?? "",
                has_minimap = img.Node.FindNodeByPath(@"miniMap\canvas") != null,
                link,
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void MobGrid(string mobWzPath, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(mobWzPath);
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        if (!linker.Load(str.wz_files[0], null, null))
        {
            Console.WriteLine("{\"error\":\"string linker load failed\"}");
            return;
        }
        var images = new Dictionary<string, Wz_Image>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            if (n.Value is Wz_Image img && n.Text.EndsWith(".img"))
                images[n.Text] = img;
        }
        var rows = new List<object>();
        foreach (var kv in linker.StringMob)
        {
            var id = kv.Key.ToString().PadLeft(7, '0');
            var imageName = id + ".img";
            if (!images.TryGetValue(imageName, out var img)) continue;
            img.TryExtract();
            var node = img.Node;
            var linkNode = node.FindNodeByPath(@"info\link");
            int? link = linkNode?.Value == null ? (int?)null : Convert.ToInt32(linkNode.Value);
            rows.Add(new {
                id,
                name = kv.Value.Name ?? "",
                has_stand = node.Nodes["stand"] != null,
                has_fly = node.Nodes["fly"] != null,
                has_move = node.Nodes["move"] != null,
                has_hit = node.Nodes["hit1"] != null || node.Nodes["hit"] != null,
                has_die = node.Nodes["die1"] != null || node.Nodes["die"] != null,
                has_thumbnail = node.FindNodeByPath(@"info\thumbnail") != null,
                link,
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void SkillGrid(string skillWzPath, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(skillWzPath);
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        if (!linker.Load(str.wz_files[0], null, null))
        {
            Console.WriteLine("{\"error\":\"string linker load failed\"}");
            return;
        }
        var rows = new List<object>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            var book = n.Text.Replace(".img", "");
            if (book.Length == 0 || !book.All(char.IsDigit)) continue;
            img.TryExtract();
            linker.StringSkill2.TryGetValue(book, out var bookSr);
            rows.Add(new {
                id = book,
                row_type = "book",
                book,
                name = bookSr?["bookName"] ?? "",
                desc = "",
                has_icon = img.Node.FindNodeByPath(@"info\icon") != null,
                has_common = false,
                h_count = 0,
            });
            var skillRoot = img.Node.Nodes["skill"];
            if (skillRoot == null) continue;
            foreach (Wz_Node child in skillRoot.Nodes)
            {
                int id;
                if (!int.TryParse(child.Text, out id)) continue;
                linker.StringSkill.TryGetValue(id, out var sr);
                var ss = sr as StringResultSkill;
                rows.Add(new {
                    id = child.Text,
                    row_type = "skill",
                    book,
                    name = sr?.Name ?? "",
                    desc = sr?.Desc ?? "",
                    has_icon = child.Nodes["icon"] != null,
                    has_common = child.Nodes["common"] != null,
                    h_count = ss?.SkillH.Count ?? 0,
                });
            }
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void Main(string[] args)
    {
        if (args[0] == "fixture") Fixture(args[1]);
        else if (args[0] == "mapnodes") MapNodes(args[1], args[2]);
        else if (args[0] == "nodepaths") NodePaths(args[1], args[2]);
        else if (args[0] == "gear") GearTooltip(args[1], args[2]);
        else if (args[0] == "db2") Db2(args[1], args[2]);
        else if (args[0] == "icons") Icons(args[1], args[2]);
        else if (args[0] == "items") ItemGrid(args[1], args[2], args[3]);
        else if (args[0] == "maps") MapGrid(args[1], args[2]);
        else if (args[0] == "mobs") MobGrid(args[1], args[2]);
        else if (args[0] == "skills") SkillGrid(args[1], args[2]);
        else if (args[0] == "set") SetTooltip(args[1], int.Parse(args[2]));
        else if (args[0] == "potential") PotentialTooltip(args[1], int.Parse(args[2]), int.Parse(args[3]));
        else Real(args[1]);
    }
}
