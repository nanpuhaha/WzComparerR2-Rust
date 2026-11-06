using System.Text.Json;
using System.Security.Cryptography;
using WzComparerR2.WzLib;
using WzComparerR2.Common;
using WzComparerR2.CharaSim;
using System.Globalization;
using System.Text;

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
    static void NpcGrid(string npcWzPath, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(npcWzPath);
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
            var id = n.Text.Replace(".img", "");
            if (id.Length == 0 || !id.All(char.IsDigit)) continue;
            img.TryExtract();
            int idNum;
            if (!int.TryParse(id, out idNum)) continue;
            linker.StringNpc.TryGetValue(idNum, out var sr);
            var linkNode = img.Node.FindNodeByPath(@"info\link");
            int? link = linkNode?.Value == null ? (int?)null : Convert.ToInt32(linkNode.Value);
            rows.Add(new {
                id,
                name = sr?.Name ?? "",
                has_stand_0 = img.Node.FindNodeByPath(@"stand\0") != null,
                has_default_0 = img.Node.FindNodeByPath(@"default\0") != null,
                has_link = linkNode != null,
                link,
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ReactorGrid(string reactorWzPath)
    {
        var st = new Wz_Structure(); st.Load(reactorWzPath);
        var rows = new List<object>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            var id = n.Text.Replace(".img", "");
            if (id.Length == 0 || !id.All(char.IsDigit)) continue;
            img.TryExtract();
            var info = img.Node.Nodes["info"];
            var linkNode = info?.Nodes["link"];
            int? link = linkNode?.Value == null ? (int?)null : Convert.ToInt32(linkNode.Value);
            rows.Add(new {
                id,
                has_icon = img.Node.FindNodeByPath(@"0\0") != null,
                has_info = info != null,
                has_link = linkNode != null,
                link,
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void AddMusic(Wz_Node node, string prefix, List<object> rows)
    {
        foreach (Wz_Node child in node.Nodes)
        {
            var path = prefix + "/" + child.Text;
            if (child.Value is Wz_Sound snd)
            {
                rows.Add(new { path, data_len = snd.DataLength, duration_ms = snd.Ms });
            }
            else if (child.Nodes.Count > 0)
            {
                AddMusic(child, path, rows);
            }
        }
    }
    static bool IsMusicImage(string name) =>
        name.StartsWith("Bgm") || name.StartsWith("PL_3") || name.StartsWith("PL_B")
        || name.StartsWith("PL_C") || name.StartsWith("PL_M");
    static void MusicGrid(string soundWzPath)
    {
        var st = new Wz_Structure(); st.Load(soundWzPath);
        var root = st.WzNode.Text;
        var rows = new List<object>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            if (!IsMusicImage(n.Text)) continue;
            img.TryExtract();
            AddMusic(img.Node, root + "/" + n.Text, rows);
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("path").GetValue(a),
            (string)b.GetType().GetProperty("path").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static bool IsDamageSkinName(string name) =>
        name.Contains("스킨") || name.Contains("Damage Skin") || name.Contains("字型")
        || name.Contains("ジスキン") || name.Contains("伤害皮肤");
    static void DamageSkinGrid(string consumeWzPath, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(consumeWzPath);
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        if (!linker.Load(str.wz_files[0], null, null))
        {
            Console.WriteLine("{\"error\":\"string linker load failed\"}");
            return;
        }
        var images = new List<(string name, Wz_Node node)>();
        foreach (var name in new[] { "0243.img", "0263.img" })
        {
            var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == name);
            if (imgNode?.Value is Wz_Image img && img.TryExtract())
                images.Add((name.Replace(".img", ""), img.Node));
        }
        var rows = new List<object>();
        foreach (var kv in linker.StringItem)
        {
            var sr = kv.Value;
            if (!(sr.FullPath ?? "").Contains("Consume.img")) continue;
            if (!IsDamageSkinName(sr.Name ?? "")) continue;
            var id = "0" + kv.Key.ToString();
            string iconImage = null;
            bool hasSample = false;
            foreach (var (imageName, imageNode) in images)
            {
                var item = imageNode.Nodes[id];
                var info = item?.Nodes["info"];
                if (info == null) continue;
                if (info.Nodes["icon"] != null) iconImage = imageName;
                if (info.Nodes["sample"] != null) hasSample = true;
            }
            rows.Add(new {
                id,
                name = sr.Name ?? "",
                desc = sr.Desc ?? "",
                icon_image = iconImage,
                has_sample = hasSample,
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void MorphGrid(string consumeWzPath, string morphWzPath, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(consumeWzPath);
        var morphSt = new Wz_Structure(); morphSt.Load(morphWzPath);
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        if (!linker.Load(str.wz_files[0], null, null))
        {
            Console.WriteLine("{\"error\":\"string linker load failed\"}");
            return;
        }
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == "0221.img");
        if (imgNode?.Value is not Wz_Image items || !items.TryExtract())
        {
            Console.WriteLine("[]");
            return;
        }
        var morphImages = new Dictionary<string, Wz_Image>();
        foreach (Wz_Node n in morphSt.WzNode.Nodes)
        {
            if (n.Value is Wz_Image m && n.Text.EndsWith(".img"))
                morphImages[n.Text] = m;
        }
        var rows = new List<object>();
        foreach (Wz_Node item in items.Node.Nodes)
        {
            int idNum;
            if (!int.TryParse(item.Text, out idNum)) continue;
            linker.StringItem.TryGetValue(idNum, out var sr);
            var morphRaw = item.FindNodeByPath(@"spec\morph")?.Value;
            var morphId = (morphRaw == null ? "" : Convert.ToString(morphRaw)).PadLeft(4, '0');
            bool hasMorph = false;
            if (morphImages.TryGetValue(morphId + ".img", out var morphImage) && morphImage.TryExtract())
                hasMorph = morphImage.Node.FindNodeByPath(@"walk\0") != null;
            rows.Add(new {
                id = item.Text,
                name = sr?.Name ?? "",
                desc = sr?.Desc ?? "",
                morph_id = morphId,
                has_icon = item.FindNodeByPath(@"info\icon") != null,
                has_morph = hasMorph,
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static int ValueInt(Wz_Node node, string key)
    {
        var c = node?.Nodes[key];
        return c?.Value == null ? 0 : Convert.ToInt32(c.Value);
    }
    static string ValueStr(Wz_Node node, string key)
    {
        var c = node?.Nodes[key];
        return c?.Value == null ? "" : Convert.ToString(c.Value);
    }
    static void TamingMobGrid(string tamingWzPath, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(tamingWzPath);
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
            if (!n.Text.StartsWith("0")) continue;
            var id = n.Text.Replace(".img", "");
            int idNum;
            if (!int.TryParse(id, out idNum)) continue;
            img.TryExtract();
            linker.StringEqp.TryGetValue(idNum, out var sr);
            var info = img.Node.Nodes["info"];
            rows.Add(new {
                id,
                name = sr?.Name ?? "",
                desc = sr?.Desc ?? "",
                h1 = sr?["h1"] ?? "",
                has_icon = info?.Nodes["icon"] != null,
                req_level = ValueInt(info, "reqLevel"),
                req_job = ValueInt(info, "reqJob"),
                tuc = ValueInt(info, "tuc"),
                islot = ValueStr(info, "islot"),
                taming_mob = ValueInt(info, "tamingMob"),
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void CharacterGrid(string characterWzPath, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(characterWzPath);
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
            if (!n.Text.StartsWith("0")) continue;
            var id = n.Text.Replace(".img", "");
            int idNum;
            if (!int.TryParse(id, out idNum)) continue;
            img.TryExtract();
            linker.StringEqp.TryGetValue(idNum, out var sr);
            var info = img.Node.Nodes["info"];
            var defaultNode = img.Node.Nodes["default"];
            var category = new DirectoryInfo(Path.GetDirectoryName(characterWzPath)).Name;
            bool hasIcon = category switch {
                "Hair" => defaultNode?.Nodes["hairOverHead"] != null,
                "Face" => defaultNode?.Nodes["face"] != null,
                _ => info?.Nodes["icon"] != null,
            };
            rows.Add(new {
                id,
                name = sr?.Name ?? "",
                desc = sr?.Desc ?? "",
                h1 = sr?["h1"] ?? "",
                has_icon = hasIcon,
                req_level = ValueInt(info, "reqLevel"),
                req_job = ValueInt(info, "reqJob"),
                tuc = ValueInt(info, "tuc"),
                islot = ValueStr(info, "islot"),
                vslot = ValueStr(info, "vslot"),
                inc_str = ValueInt(info, "incSTR"),
                inc_dex = ValueInt(info, "incDEX"),
                inc_int = ValueInt(info, "incINT"),
                inc_luk = ValueInt(info, "incLUK"),
                inc_pad = ValueInt(info, "incPAD"),
                inc_mad = ValueInt(info, "incMAD"),
                inc_pdd = ValueInt(info, "incPDD"),
                inc_mdd = ValueInt(info, "incMDD"),
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    sealed class NoSkillResolver : WzComparerR2.CharaSim.ISkillPropertyResolver
    {
        public bool TryResolve(string propertyName, int? skillId, int? skillLevel,
            out WzComparerR2.CharaSim.ResolvedSkillProperty value)
        {
            value = default;
            return false;
        }
    }
    static void SkillSummary(string stringWzPath, int limit)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        if (!linker.Load(str.wz_files[0], null, null))
        {
            Console.WriteLine("{\"error\":\"string linker load failed\"}");
            return;
        }
        var resolver = new NoSkillResolver();
        var rows = new List<object>();
        foreach (var kv in linker.StringSkill2.OrderBy(k => k.Key, StringComparer.Ordinal))
        {
            var sr = kv.Value as StringResultSkill;
            if (sr == null) continue;
            if (!string.IsNullOrEmpty(sr.Desc))
            {
                rows.Add(new {
                    id = kv.Key,
                    kind = "desc",
                    summary = WzComparerR2.CharaSim.SummaryParser.GetSkillSummary(
                        sr.Desc, 1, resolver, WzComparerR2.CharaSim.SummaryParams.Default),
                });
            }
            for (int i = 0; i < sr.SkillH.Count; i++)
            {
                if (string.IsNullOrEmpty(sr.SkillH[i])) continue;
                rows.Add(new {
                    id = kv.Key,
                    kind = "h" + i.ToString(),
                    summary = WzComparerR2.CharaSim.SummaryParser.GetSkillSummary(
                        sr.SkillH[i], 1, resolver, WzComparerR2.CharaSim.SummaryParams.Default),
                });
            }
            if (rows.Count >= limit) break;
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemTooltip(string itemWzPath, string imageName, string itemId, string stringWzPath)
    {
        var st = new Wz_Structure(); st.Load(itemWzPath);
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        var img = imgNode?.Value as Wz_Image;
        if (img == null) { Console.WriteLine("{\"error\":\"image not found\"}"); return; }
        img.TryExtract();
        var item = img.Node.Nodes[itemId];
        if (item == null) { Console.WriteLine("{\"error\":\"item not found\"}"); return; }
        var info = item.Nodes["info"];
        var props = new Dictionary<ItemPropType, long>();
        if (info != null)
            foreach (Wz_Node c in info.Nodes)
            {
                if (c.Value == null || !Enum.TryParse(c.Text, out ItemPropType type)) continue;
                try { props[type] = Convert.ToInt64(c.Value); } catch { }
            }
        var tags = new List<string>();
        long v;
        if (props.TryGetValue(ItemPropType.only, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.only, v));
        if (props.TryGetValue(ItemPropType.quest, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.quest, v));
        if (props.TryGetValue(ItemPropType.pquest, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.pquest, v));
        if (props.TryGetValue(ItemPropType.tradeBlock, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.tradeBlock, v));
        if (props.TryGetValue(ItemPropType.useTradeBlock, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.useTradeBlock, v));
        else if (int.Parse(itemId) / 10000 == 501 || int.Parse(itemId) / 10000 == 502 || int.Parse(itemId) / 10000 == 516)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.tradeBlock, 1));
        if (props.TryGetValue(ItemPropType.accountSharable, out v) && v != 0)
        {
            long v2;
            if (props.TryGetValue(ItemPropType.exp_minLev, out v2) && v2 != 0)
                tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.useTradeBlock, 1));
            if (props.TryGetValue(ItemPropType.sharableOnce, out v2) && v2 != 0)
                tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.sharableOnce, v2));
            else
                tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.accountSharable, v));
        }
        if (props.TryGetValue(ItemPropType.exchangeableOnce, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.exchangeableOnce, v));
        if (props.TryGetValue(ItemPropType.multiPet, out v))
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.multiPet, v));
        else if (int.Parse(itemId) / 10000 == 500)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.multiPet, 0));
        if (props.TryGetValue(ItemPropType.accountSharableAfterExchange, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.accountSharableAfterExchange, v));
        if (props.TryGetValue(ItemPropType.mintable, out v))
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.mintable, v));
        tags = tags.Where(t => !string.IsNullOrEmpty(t)).ToList();

        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, null);
        linker.StringItem.TryGetValue(int.Parse(itemId), out var sr);
        Console.WriteLine(JsonSerializer.Serialize(new {
            id = itemId,
            name = sr?.Name ?? "",
            desc = sr?.Desc ?? "",
            auto_desc = sr?.AutoDesc ?? "",
            attributes = tags,
        }, new JsonSerializerOptions { WriteIndented = true }));
    }
    static Wz_Node OpenStringChild(Wz_Structure str, string dir)
    {
        var imageName = dir == "Install" ? "Ins.img" : dir + ".img";
        var imgNode = str.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        if (imgNode?.Value is not Wz_Image img || !img.TryExtract()) return null;
        var node = img.Node;
        return dir == "Etc" ? node.Nodes["Etc"] : node;
    }
    static string StrField(Wz_Node child, string idString, string field)
    {
        var found = child?.FindNodeByPath(false, idString, field);
        return found?.Value?.ToString() ?? "  ";
    }
    static string OwnValue(Wz_Node node, string field)
    {
        return node.Nodes[field]?.Value?.ToString();
    }
    static HashSet<string> ReadCashPackages(string dataRoot)
    {
        var set = new HashSet<string>();
        foreach (var file in Directory.GetFiles(Path.Combine(dataRoot, "Etc"), "*.wz").OrderBy(x => x))
        {
            var st = new Wz_Structure(); st.Load(file);
            var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == "CashPackage.img");
            if (imgNode?.Value is Wz_Image img && img.TryExtract())
                foreach (Wz_Node c in img.Node.Nodes) set.Add(c.Text);
        }
        return set;
    }
    static List<string> BuildItemAttributes(Wz_Node info, int itemId)
    {
        var props = new Dictionary<ItemPropType, long>();
        if (info != null)
            foreach (Wz_Node c in info.Nodes)
            {
                if (c.Value == null || !Enum.TryParse(c.Text, out ItemPropType type)) continue;
                try { props[type] = Convert.ToInt64(c.Value); } catch { }
            }
        var tags = new List<string>();
        long v;
        if (props.TryGetValue(ItemPropType.only, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.only, v));
        if (props.TryGetValue(ItemPropType.quest, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.quest, v));
        if (props.TryGetValue(ItemPropType.pquest, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.pquest, v));
        if (props.TryGetValue(ItemPropType.tradeBlock, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.tradeBlock, v));
        if (props.TryGetValue(ItemPropType.useTradeBlock, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.useTradeBlock, v));
        else if (itemId / 10000 == 501 || itemId / 10000 == 502 || itemId / 10000 == 516)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.tradeBlock, 1));
        if (props.TryGetValue(ItemPropType.accountSharable, out v) && v != 0)
        {
            long v2;
            if (props.TryGetValue(ItemPropType.exp_minLev, out v2) && v2 != 0)
                tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.useTradeBlock, 1));
            if (props.TryGetValue(ItemPropType.sharableOnce, out v2) && v2 != 0)
                tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.sharableOnce, v2));
            else
                tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.accountSharable, v));
        }
        if (props.TryGetValue(ItemPropType.exchangeableOnce, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.exchangeableOnce, v));
        if (props.TryGetValue(ItemPropType.multiPet, out v))
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.multiPet, v));
        else if (itemId / 10000 == 500)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.multiPet, 0));
        if (props.TryGetValue(ItemPropType.accountSharableAfterExchange, out v) && v != 0)
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.accountSharableAfterExchange, v));
        if (props.TryGetValue(ItemPropType.mintable, out v))
            tags.Add(ItemStringHelper.GetItemPropString(ItemPropType.mintable, v));
        return tags.Where(t => !string.IsNullOrEmpty(t)).ToList();
    }
    static void ItemTab(string dataRoot, string dir, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var special = dir == "Special";
        var childNode = special ? null : OpenStringChild(str, dir);
        if (!special && childNode == null)
        {
            Console.WriteLine("{\"error\":\"string image not found\"}");
            return;
        }
        var cash = special ? ReadCashPackages(dataRoot) : new HashSet<string>();
        var rows = new List<object>();
        foreach (var file in Directory.GetFiles(Path.Combine(dataRoot, "Item", dir), "*.wz").OrderBy(x => x))
        {
            var st = new Wz_Structure(); st.Load(file);
            foreach (Wz_Node n in st.WzNode.Nodes)
            {
                if (n.Value is not Wz_Image img) continue;
                if (special && n.Text != "0910.img") continue;
                if (!img.TryExtract()) continue;
                if (dir == "Pet")
                {
                    var petId = n.Text.Replace(".img", "");
                    rows.Add(new {
                        id = petId,
                        name = StrField(childNode, petId, "name"),
                        desc = StrField(childNode, petId, "desc"),
                        has_icon = img.Node.FindNodeByPath(@"info\iconD") != null,
                    });
                    continue;
                }
                foreach (Wz_Node item in img.Node.Nodes)
                {
                    if (special && !cash.Contains(item.Text)) continue;
                    int idNum;
                    if (!int.TryParse(item.Text, out idNum)) continue;
                    var idString = idNum.ToString();
                    string name, desc;
                    bool hasIcon;
                    if (special)
                    {
                        name = OwnValue(item, "name") ?? "  ";
                        desc = OwnValue(item, "desc") ?? "  ";
                        hasIcon = item.Nodes["icon"] != null;
                    }
                    else
                    {
                        name = StrField(childNode, idString, "name");
                        desc = StrField(childNode, idString, "desc");
                        hasIcon = item.FindNodeByPath(@"info\icon") != null;
                    }
                    rows.Add(new { id = item.Text, name, desc, has_icon = hasIcon });
                }
            }
        }
        var deduped = rows
            .GroupBy(r => (string)r.GetType().GetProperty("id").GetValue(r))
            .Select(g => g.First())
            .OrderBy(r => (string)r.GetType().GetProperty("id").GetValue(r), StringComparer.Ordinal)
            .ToList();
        Console.WriteLine(JsonSerializer.Serialize(deduped, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemTooltips(string dataRoot, string dir, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var special = dir == "Special";
        var childNode = special ? null : OpenStringChild(str, dir);
        if (!special && childNode == null)
        {
            Console.WriteLine("{\"error\":\"string image not found\"}");
            return;
        }
        var cash = special ? ReadCashPackages(dataRoot) : new HashSet<string>();
        var rows = new List<object>();
        foreach (var file in Directory.GetFiles(Path.Combine(dataRoot, "Item", dir), "*.wz").OrderBy(x => x))
        {
            var st = new Wz_Structure(); st.Load(file);
            foreach (Wz_Node n in st.WzNode.Nodes)
            {
                if (n.Value is not Wz_Image img) continue;
                if (special && n.Text != "0910.img") continue;
                if (!img.TryExtract()) continue;
                if (dir == "Pet")
                {
                    var petId = n.Text.Replace(".img", "");
                    rows.Add(new {
                        id = petId,
                        name = StrField(childNode, petId, "name"),
                        desc = StrField(childNode, petId, "desc"),
                        attributes = BuildItemAttributes(img.Node.Nodes["info"], int.Parse(petId)),
                    });
                    continue;
                }
                foreach (Wz_Node item in img.Node.Nodes)
                {
                    if (special && !cash.Contains(item.Text)) continue;
                    int idNum;
                    if (!int.TryParse(item.Text, out idNum)) continue;
                    var idString = idNum.ToString();
                    string name, desc;
                    if (special)
                    {
                        name = OwnValue(item, "name") ?? "  ";
                        desc = OwnValue(item, "desc") ?? "  ";
                    }
                    else
                    {
                        name = StrField(childNode, idString, "name");
                        desc = StrField(childNode, idString, "desc");
                    }
                    rows.Add(new {
                        id = item.Text,
                        name,
                        desc,
                        attributes = BuildItemAttributes(item.Nodes["info"], idNum),
                    });
                }
            }
        }
        var deduped = rows
            .GroupBy(r => (string)r.GetType().GetProperty("id").GetValue(r))
            .Select(g => g.First())
            .OrderBy(r => (string)r.GetType().GetProperty("id").GetValue(r), StringComparer.Ordinal)
            .ToList();
        Console.WriteLine(JsonSerializer.Serialize(deduped, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void GearProps(string characterWzPath)
    {
        var st = new Wz_Structure(); st.Load(characterWzPath);
        var seen = new HashSet<string>();
        var rows = new List<object>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            if (!img.TryExtract()) continue;
            var info = img.Node.Nodes["info"];
            if (info == null) continue;
            foreach (Wz_Node c in info.Nodes)
            {
                if (c.Value == null) continue;
                long value;
                try { value = Convert.ToInt64(c.Value); } catch { continue; }
                GearPropType type;
                var parsed = Enum.TryParse(c.Text, out type);
                var text = parsed
                    ? ItemStringHelper.GetGearPropString(type, value)
                    : null;
                var key = c.Text + "\t" + value;
                if (!seen.Add(key)) continue;
                rows.Add(new { key = c.Text, value, text });
            }
        }
        rows.Sort((a, b) =>
        {
            var ka = (string)a.GetType().GetProperty("key").GetValue(a);
            var kb = (string)b.GetType().GetProperty("key").GetValue(b);
            var cmp = string.CompareOrdinal(ka, kb);
            if (cmp != 0) return cmp;
            return ((long)a.GetType().GetProperty("value").GetValue(a))
                .CompareTo((long)b.GetType().GetProperty("value").GetValue(b));
        });
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void OpenApi(string text)
    {
        byte[] dec = null;
        try { dec = WzComparerR2.OpenAPI.Utils.Decrypt(text); } catch { }
        string itemId = null;
        try { itemId = WzComparerR2.OpenAPI.Utils.GetItemID(text); } catch { }
        Console.WriteLine(JsonSerializer.Serialize(new {
            decrypt = dec == null ? null : Convert.ToHexString(dec).ToLowerInvariant(),
            decrypt_len = dec?.Length ?? 0,
            check_ver = dec == null ? 0 : WzComparerR2.OpenAPI.Utils.CheckVer(dec),
            item_id = itemId,
        }, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void NetworkRc4(string networkDllPath, string keyHex, string dataHex)
    {
        var asm = System.Reflection.Assembly.LoadFrom(networkDllPath);
        var type = asm.GetType("WzComparerR2.Network.RC4CryptoServiceProvider");
        var rc4 = (System.Security.Cryptography.SymmetricAlgorithm)Activator.CreateInstance(type);
        rc4.Key = Convert.FromHexString(keyHex);
        var encryptor = rc4.CreateEncryptor();
        var data = Convert.FromHexString(dataHex);
        var outBytes = encryptor.TransformFinalBlock(data, 0, data.Length);
        Console.WriteLine(Convert.ToHexString(outBytes).ToLowerInvariant());
    }
    static void JobNames(int max)
    {
        var rows = new List<object>();
        for (int code = 0; code <= max; code++)
        {
            var name = ItemStringHelper.GetJobName(code);
            if (name != null) rows.Add(new { code, name });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void CoreSpecs()
    {
        var rows = new List<object>();
        var descs = new string[] { null, "", "테스트", "사과", "고양이", "ABC" };
        foreach (ItemCoreSpecType kind in Enum.GetValues(typeof(ItemCoreSpecType)))
        {
            foreach (var value in new[] { 0, 1, 5, 100 })
            {
                foreach (var desc in descs)
                {
                    rows.Add(new {
                        kind = kind.ToString(),
                        value,
                        desc,
                        text = ItemStringHelper.GetItemCoreSpecString(kind, value, desc),
                    });
                }
            }
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void FifthJobNames()
    {
        int[] skills = { 0, 10000, 11200, 2200, 14200, 16400, 40000, 50000, 23200 };
        var lists = new List<int[]>
        {
            Array.Empty<int>(),
            new[] { 0 },
            new[] { 112 },
            new[] { 1120, 1210 },
            new[] { 5100, 5200 },
            new[] { 2000, 3100 },
            new[] { 1120, 0 },
            new[] { 2100 },
            new[] { 2200, 2300 },
            new[] { 3100, 3200 },
            new[] { 4100, 4200 },
            new[] { 6100, 6200 },
            new[] { 10100 },
            new[] { 11100 },
            new[] { 12100 },
            new[] { 13100 },
            new[] { 14100 },
            new[] { 15100 },
            new[] { 16100 },
            new[] { 17100 },
            new[] { 18100 },
            new[] { 19100 },
        };
        var rows = new List<object>();
        foreach (var skill in skills)
            foreach (var list in lists)
            {
                string text = null;
                try { text = ItemStringHelper.GetFifthJobName(skill, list.ToList()); } catch { }
                rows.Add(new { skill, job_id = list, text });
            }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void AchievementTooltip(string wzPath)
    {
        var st = new Wz_Structure(); st.Load(wzPath);
        var achievements = new List<WzComparerR2.CharaSim.Achievement>();
        var names = new Dictionary<int, string>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            if (!img.TryExtract()) continue;
            var ac = WzComparerR2.CharaSim.Achievement.CreateFromNode(img.Node, null);
            if (ac == null) continue;
            achievements.Add(ac);
            names[ac.ID] = img.Node.FindNodeByPath(@"info\name")?.Value?.ToString() ?? "";
        }
        var rows = new List<object>();
        foreach (var ac in achievements.OrderBy(a => a.ID))
        {
            var info = PluginBaseFind(st, ac.ID);
            var name = names.TryGetValue(ac.ID, out var nm) ? nm : "";
            var desc = info?.FindNodeByPath(@"desc")?.Value?.ToString() ?? "";
            var lines = new List<string> { name };
            if (!string.IsNullOrEmpty(ac.Start) && !string.IsNullOrEmpty(ac.End)
                && DateTime.TryParseExact(ac.Start, "yyyyMMddHHmmss", CultureInfo.InvariantCulture, DateTimeStyles.None, out var start)
                && DateTime.TryParseExact(ac.End, "yyyyMMddHHmmss", CultureInfo.InvariantCulture, DateTimeStyles.None, out var end))
            {
                var fmt = "yyyy년 MM월 dd일 HH시 mm분";
                lines.Add($"{start.ToString(fmt)} ~ {end.ToString(fmt)}");
            }
            if (!string.IsNullOrEmpty(desc)) lines.Add(desc);
            if (ac.ShowMissions) lines.AddRange(ac.Missions);
            var category = string.Join(" / ", new[] { ac.MainCategory, ac.SubCategory }.Where(t => !string.IsNullOrEmpty(t)));
            if (!string.IsNullOrEmpty(category)) lines.Add($"카테고리: {category}");
            if (ac.PriorIDs.Count > 0)
            {
                var list = string.Join(", ", ac.PriorIDs.Select(id =>
                    $"{(names.TryGetValue(id, out var pn) ? pn : "null")}({id:D5})"));
                lines.Add($"선행 업적: {list}");
            }
            if (ac.Hide) lines.Add("업적창에 표시되지 않음");
            rows.Add(new {
                id = ac.ID,
                name,
                desc,
                score = ac.Score,
                difficulty = ac.Difficulty,
                ui_form = ac.UiForm,
                block = ac.Block,
                main_category = ac.MainCategory,
                sub_category = ac.SubCategory,
                prior_ids = ac.PriorIDs,
                missions = ac.Missions,
                rewards = ac.Rewards.Select(r => new { id = r.ID, desc = r.Desc }),
                lines,
            });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static Wz_Node PluginBaseFind(Wz_Structure st, int id)
    {
        var n = st.WzNode.Nodes.FirstOrDefault(x => x.Text == id + ".img");
        return n?.Value is Wz_Image img && img.TryExtract() ? img.Node.FindNodeByPath("info") : null;
    }
    static string[] SplitWorldArchive(string orgText)
    {
        return orgText.Split(new string[] { "\r\n", "\\r\\n", "\\r", "\\n", "\r", "\n" }, StringSplitOptions.None);
    }
    static void WorldArchive(string stringWzPath, string etcWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var etc = new Wz_Structure(); etc.Load(etcWzPath);
        var linker = new StringLinker();
        if (!linker.Load(str.wz_files[0], null, etc.wz_files[0]))
        {
            Console.WriteLine("{\"error\":\"linker load failed\"}");
            return;
        }
        var rows = new List<object>();
        foreach (var kv in linker.StringWorldArchiveMob.OrderBy(k => k.Key))
        {
            var desc = kv.Value.Desc;
            linker.StringMonsterBook.TryGetValue(kv.Key, out var book);
            var lines = new List<string>();
            if (!string.IsNullOrEmpty(desc)) lines.AddRange(SplitWorldArchive(desc));
            if (!string.IsNullOrEmpty(book?.Desc))
            {
                lines.Add("[몬스터북]");
                lines.AddRange(SplitWorldArchive(book.Desc));
            }
            rows.Add(new {
                kind = "mob",
                id = kv.Key,
                desc,
                monster_book = book?.Desc,
                lines,
            });
        }
        foreach (var kv in linker.StringWorldArchiveNpc.OrderBy(k => k.Key))
        {
            var desc = kv.Value.Desc;
            var quote = WzComparerR2.CharaSim.NpcQuote.CreateFromNode(kv.Key, linker);
            string message = null;
            if (quote != null)
            {
                var set = new HashSet<string>();
                foreach (var q in quote.NQuote) set.Add(q.Value);
                foreach (var q in quote.FQuote) set.Add(q.Value);
                foreach (var q in quote.WQuote) set.Add(q.Value);
                foreach (var q in quote.DQuote) set.Add(q.Value);
                foreach (var q in quote.SpecialQuote) set.Add(q.Value);
                message = string.Join("\r\n", set);
                if (message.Length == 0) message = null;
            }
            var lines = new List<string>();
            var hasQuote = !string.IsNullOrEmpty(message);
            if (!string.IsNullOrEmpty(desc)) lines.AddRange(SplitWorldArchive(desc));
            else if (hasQuote) lines.Add("(정보 없음)");
            if (hasQuote)
            {
                lines.Add("NPC 대사");
                foreach (var line in SplitWorldArchive(message))
                {
                    var trimmed = line.Trim();
                    if (trimmed.Length != 0) lines.Add(" - " + trimmed);
                }
            }
            rows.Add(new { kind = "npc", id = kv.Key, desc, quote = message, lines });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static string ClassifyNode(Wz_Node node)
    {
        if (node == null) return "missing";
        object v = node.Value;
        return v switch
        {
            null => "null",
            int i => $"int:{i}",
            short sh => $"int:{sh}",
            long l => $"int:{l}",
            uint ui => $"int:{ui}",
            float f => $"float:{f}",
            double db => $"float:{db}",
            string s => $"string:{s}",
            Wz_Png => "canvas",
            Wz_Sound => "sound",
            Wz_Uol u => "other",
            _ => "other",
        };
    }
    static void FindWz(string[] args, int start)
    {
        var maps = new Dictionary<string, Wz_Structure>(StringComparer.OrdinalIgnoreCase);
        int i = start;
        while (i < args.Length && args[i] != "--")
        {
            var sp = args[i].Split('=', 2);
            var st = new Wz_Structure();
            st.Load(sp[1]);
            maps[sp[0]] = st;
            i++;
        }
        i++;
        var rows = new List<object>();
        for (; i < args.Length; i++)
        {
            var fullPath = args[i];
            var parts = fullPath.Split('/', '\\');
            Wz_Node found = null;
            if (parts.Length > 0 && maps.TryGetValue(parts[0], out var structure))
            {
                found = parts.Length <= 1
                    ? structure.WzNode
                    : structure.WzNode.FindNodeByPath(true, parts.Skip(1).ToArray());
            }
            rows.Add(new {
                path = fullPath,
                found = found != null,
                name = found?.Text,
                value = found == null ? null : ClassifyNode(found),
                children = found?.Nodes.Count ?? 0,
            });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    sealed class NetBinder : Newtonsoft.Json.Serialization.ISerializationBinder
    {
        public void BindToName(Type serializedType, out string assemblyName, out string typeName)
        {
            var attr = (Newtonsoft.Json.JsonObjectAttribute)Attribute.GetCustomAttribute(serializedType, typeof(Newtonsoft.Json.JsonObjectAttribute));
            typeName = attr?.Id ?? serializedType.FullName;
            assemblyName = null;
        }
        public Type BindToType(string assemblyName, string typeName) => null;
    }
    static void NetPack(string networkDllPath)
    {
        var asm = System.Reflection.Assembly.LoadFrom(networkDllPath);
        var settings = new Newtonsoft.Json.JsonSerializerSettings
        {
            TypeNameHandling = Newtonsoft.Json.TypeNameHandling.Objects,
            SerializationBinder = new NetBinder(),
            Converters = new Newtonsoft.Json.JsonConverter[] { new NetByteArrayConverter() },
        };
        object Make(string name, params (string prop, object value)[] props)
        {
            var t = asm.GetType("WzComparerR2.Network.Contracts." + name);
            var o = Activator.CreateInstance(t);
            foreach (var (prop, value) in props) t.GetProperty(prop).SetValue(o, value);
            return o;
        }
        var chatGroupPublic = Enum.Parse(asm.GetType("WzComparerR2.Network.Contracts.ChatGroup"), "Public");
        var packets = new (string label, object pack)[]
        {
            ("heartbeat", Make("PackHeartBeat")),
            ("loginreq", Make("PackLoginReq", ("WcID", "wc-01"), ("NickName", "테스터"))),
            ("loginresp", Make("PackLoginResp", ("SessionID", "sess-42"))),
            ("sendchat", Make("PackSendChat", ("Message", "안녕"), ("Group", chatGroupPublic))),
            ("sendchat-null", Make("PackSendChat", ("Message", null), ("Group", chatGroupPublic))),
            ("cryptreq", Make("PackCryptReq", ("Exponent", new byte[] { 1, 2, 3 }), ("Modulus", new byte[] { 9, 8, 7, 6 }))),
            ("cryptresp", Make("PackCryptResp", ("KeyEncryptedS2C", new byte[] { 1, 2 }), ("KeyEncryptedC2S", new byte[] { 3, 4 }))),
        };
        var rows = new List<object>();
        foreach (var (label, pack) in packets)
        {
            var json = Newtonsoft.Json.JsonConvert.SerializeObject(pack, settings);
            var body = Encoding.UTF8.GetBytes(json);
            var buffer = new byte[body.Length + 2];
            buffer[0] = (byte)body.Length;
            buffer[1] = (byte)(body.Length >> 8);
            body.CopyTo(buffer, 2);
            rows.Add(new { label, hex = Convert.ToHexString(buffer).ToLowerInvariant(), len = buffer.Length, json });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    sealed class NetByteArrayConverter : Newtonsoft.Json.JsonConverter
    {
        public override bool CanConvert(Type objectType) => objectType == typeof(byte[]);
        public override object ReadJson(Newtonsoft.Json.JsonReader reader, Type objectType, object existingValue, Newtonsoft.Json.JsonSerializer serializer)
            => null;
        public override void WriteJson(Newtonsoft.Json.JsonWriter writer, object value, Newtonsoft.Json.JsonSerializer serializer)
            => writer.WriteValue(Convert.ToBase64String((byte[])value));
    }
    static void CharaTooltips(string npcWzPath, string reactorWzPath, string stringWzPath, string etcWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var etc = new Wz_Structure(); etc.Load(etcWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, etc.wz_files[0]);
        var locImage = etc.WzNode.Nodes.FirstOrDefault(n => n.Text == "NpcLocation.img")?.Value as Wz_Image;
        locImage?.TryExtract();
        var rows = new List<object>();

        string MapName(int mapId)
        {
            linker.StringMap.TryGetValue(mapId, out var msr);
            return msr?.Name ?? "null";
        }

        var npc = new Wz_Structure(); npc.Load(npcWzPath);
        foreach (Wz_Node n in npc.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            var idText = n.Text.Replace(".img", "");
            if (!int.TryParse(idText, out int npcId)) continue;
            if (!img.TryExtract()) continue;
            linker.StringNpc.TryGetValue(npcId, out var sr);
            var lines = new List<string> { sr?.Name ?? "(null)", $"ID:{npcId}", "등장위치 :" };
            var locNode = locImage?.Node.Nodes[npcId.ToString()];
            int locCount = 0;
            if (locNode != null)
            {
                foreach (Wz_Node mapNode in locNode.Nodes)
                {
                    string mapName = null;
                    if (int.TryParse(mapNode.Text, out int mapId)) mapName = MapName(mapId);
                    lines.Add($" {mapName ?? "null"}({mapNode.Text})");
                    locCount++;
                }
            }
            if (locCount == 0) lines.Add(" 불명");
            rows.Add(new { kind = "npc", id = npcId, name = sr?.Name, lines });
        }

        var reactor = new Wz_Structure(); reactor.Load(reactorWzPath);
        foreach (Wz_Node n in reactor.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            if (!img.TryExtract()) continue;
            var idText = n.Text.Replace(".img", "");
            if (!int.TryParse(idText, out int rid)) continue;
            var node = img.Node;
            var info = node.Nodes["info"];
            var name = info?.Nodes["viewName"]?.Value?.ToString()
                ?? info?.Nodes["name"]?.Value?.ToString()
                ?? info?.Nodes["info"]?.Value?.ToString();
            var action = node.Nodes["action"]?.Value?.ToString();
            int clip = 0;
            while (node.Nodes[clip.ToString()] != null) clip++;
            var lines = new List<string> { $"이름: {(string.IsNullOrEmpty(name) ? "(null)" : name)}", $"클립: {clip}개" };
            if (!string.IsNullOrEmpty(action)) lines.Add($"액션: {action}");
            rows.Add(new { kind = "reactor", id = rid, name, action, clip_count = clip, lines });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static object ParseDamageSkinSection(Wz_Node node)
    {
        var keys = new List<string>();
        int numberSpace = 0, dotSpace = 0;
        bool bottomFix = false;
        foreach (Wz_Node d in node.Nodes)
        {
            if (d.Value is Wz_Uol || d.Value is Wz_Png)
            {
                keys.Add(d.Text);
                continue;
            }
            if (d.Nodes.Count > 1)
            {
                if (d.Nodes.Any(x => x.Value is Wz_Uol || x.Value is Wz_Png)) keys.Add(d.Text);
                continue;
            }
            switch (d.Text)
            {
                case "numberSpace": numberSpace = Convert.ToInt32(d.Value); break;
                case "bottomFix": bottomFix = Convert.ToInt32(d.Value) > 0; break;
                case "dotSpace": dotSpace = Convert.ToInt32(d.Value); break;
            }
        }
        keys.Sort(StringComparer.Ordinal);
        return new { keys, number_space = numberSpace, bottom_fix = bottomFix, dot_space = dotSpace };
    }
    static void AddDamageSkinEffect(Wz_Node effect, Dictionary<string, object> sections, ref string customType)
    {
        switch (effect.Text)
        {
            case "NoRed0":
            case "NoRed1":
            case "NoCri0":
            case "NoCri1":
                sections[effect.Text] = ParseDamageSkinSection(effect);
                break;
            case "NoCustom":
                foreach (Wz_Node sub in effect.Nodes)
                {
                    if (sub.Text == "customType") customType = sub.Value?.ToString();
                    else sections[sub.Text] = ParseDamageSkinSection(sub);
                }
                break;
        }
    }
    static void DamageSkinModel(string etcWzPath, string imageName)
    {
        var st = new Wz_Structure(); st.Load(etcWzPath);
        var imgNode = st.WzNode.Nodes.FirstOrDefault(n => n.Text == imageName);
        if (imgNode?.Value is not Wz_Image img || !img.TryExtract())
        {
            Console.WriteLine("{\"error\":\"image not found\"}");
            return;
        }
        var rows = new List<object>();
        foreach (Wz_Node n in img.Node.Nodes)
        {
            if (!int.TryParse(n.Text, out int id)) continue;
            string desc = null, customType = null;
            int extractId = 0;
            bool hasSample = false;
            var sections = new Dictionary<string, object>();
            foreach (Wz_Node sub in n.Nodes)
            {
                switch (sub.Text)
                {
                    case "desc": desc = sub.Value?.ToString(); break;
                    case "extractID": extractId = Convert.ToInt32(sub.Value); break;
                    case "sample": hasSample = true; break;
                    case "effect":
                        foreach (Wz_Node effect in sub.Nodes) AddDamageSkinEffect(effect, sections, ref customType);
                        break;
                    case "NoRed0":
                    case "NoRed1":
                    case "NoCri0":
                    case "NoCri1":
                    case "NoCustom":
                        AddDamageSkinEffect(sub, sections, ref customType);
                        break;
                }
            }
            rows.Add(new {
                id,
                desc,
                extract_id = extractId,
                custom_type = customType,
                has_sample = hasSample,
                sections = sections.OrderBy(kv => kv.Key, StringComparer.Ordinal)
                    .ToDictionary(kv => kv.Key, kv => kv.Value),
            });
        }
        rows.Sort((a, b) => ((int)a.GetType().GetProperty("id").GetValue(a))
            .CompareTo((int)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static object DataInfoToObj(object info)
    {
        var t = info.GetType();
        return new Dictionary<string, object>
        {
            ["name"] = t.GetProperty("Name").GetValue(info),
            ["bits"] = t.GetProperty("Bits").GetValue(info),
            ["expand_offset"] = t.GetProperty("ExpandOffset").GetValue(info),
            ["subitems"] = ((System.Collections.IEnumerable)t.GetProperty("SubItems").GetValue(info))
                .Cast<object>().Select(DataInfoToObj).ToList(),
        };
    }
    static object GetAvatarStructure(int version)
    {
        var asm = typeof(WzComparerR2.CharaSim.Item).Assembly;
        var type = asm.GetType("WzComparerR2.OpenAPI.AvatarCodeStructure");
        var dict = (System.Collections.IDictionary)type.GetField("Structure",
            System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic).GetValue(null);
        var list = (System.Collections.IEnumerable)dict[version];
        return list.Cast<object>().Select(DataInfoToObj).ToList();
    }
    static void AvatarStructure(int version)
    {
        Console.WriteLine(JsonSerializer.Serialize(GetAvatarStructure(version), new JsonSerializerOptions { WriteIndented = true }));
    }
    static void AvatarUnpack(int version, string packHex)
    {
        var asm = typeof(WzComparerR2.CharaSim.Item).Assembly;
        var unpackedType = asm.GetType("WzComparerR2.OpenAPI.UnpackedAvatarData");
        var obj = Activator.CreateInstance(unpackedType, version);
        var utils = asm.GetType("WzComparerR2.OpenAPI.Utils");
        var unpack = utils.GetMethod("Unpack");
        unpack.Invoke(null, new object[] { obj, Convert.FromHexString(packHex) });
        var list = (System.Collections.IEnumerable)unpackedType.GetProperty("Unpacked").GetValue(obj);
        var rows = list.Cast<object>().Select(i =>
        {
            var t = i.GetType();
            return new {
                name = t.GetProperty("Name").GetValue(i),
                bits = t.GetProperty("Bits").GetValue(i),
                value = t.GetProperty("Value").GetValue(i),
            };
        }).ToList();
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void MapTooltips(string mapWzPath, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, null);
        var st = new Wz_Structure(); st.Load(mapWzPath);
        var rows = new List<object>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            if (!img.TryExtract()) continue;
            var map = WzComparerR2.CharaSim.Map.CreateFromNode(img.Node, null);
            if (map == null) continue;
            linker.StringMap.TryGetValue(map.MapID, out var sr);
            if (sr == null && map.Link != null) linker.StringMap.TryGetValue(map.Link.Value, out sr);
            string MobName(int id) => linker.StringMob.TryGetValue(id, out var m) ? m.Name : null;
            string NpcName(int id) => linker.StringNpc.TryGetValue(id, out var m) ? m.Name : null;
            rows.Add(new {
                id = map.MapID,
                link = map.Link,
                barrier = map.Barrier,
                barrier_arc = map.BarrierArc,
                barrier_aut = map.BarrierAut,
                map_mark = map.MapMark,
                bgm = map.Bgm,
                street = sr?.StreetName ?? "(null)",
                map_name = sr?.MapName ?? "(null)",
                id_text = $"ID:{map.MapID}",
                desc = string.IsNullOrEmpty(sr?["mapDesc"]) ? null : sr["mapDesc"],
                bgm_text = string.IsNullOrEmpty(map.Bgm) ? null : map.Bgm,
                mobs = map.Mobs.Select(id => new object[] { id, MobName(id) ?? "(null)" }).ToList(),
                npcs = map.Npcs.Select(id => new object[] { id, NpcName(id) ?? "(null)" }).ToList(),
            });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void QuestTooltips(string questWzPath)
    {
        var st = new Wz_Structure(); st.Load(questWzPath);
        var rows = new List<object>();
        foreach (Wz_Node n in st.WzNode.Nodes)
        {
            if (n.Value is not Wz_Image img) continue;
            if (!int.TryParse(n.Text.Replace(".img", ""), out int id)) continue;
            if (!img.TryExtract()) continue;
            var node = img.Node;
            string name = null;
            var desc = new string[] { null, null, null };
            bool blocked = false;
            string demandBase = null;
            Wz_Node recommendNode = null;
            var info = node.Nodes["QuestInfo"];
            if (info != null)
            {
                name = info.Nodes["name"]?.Value?.ToString();
                for (int i = 0; i < 3; i++) desc[i] = info.Nodes[i.ToString()]?.Value?.ToString();
                blocked = Convert.ToInt32(info.Nodes["blocked"]?.Value ?? 0) == 1;
                demandBase = info.Nodes["demand"]?.Nodes["base"]?.Value?.ToString();
                recommendNode = info.Nodes["recommend"];
            }
            int lvmin = 0, lvmax = 0;
            bool lvLimit = false;
            int check1Npc = 0;
            var check1Items = new List<object>();
            var infoex = new List<object>();
            var check = node.Nodes["Check"];
            if (check?.Nodes["0"] is Wz_Node c0)
            {
                lvmin = Convert.ToInt32(c0.Nodes["lvmin"]?.Value ?? 0);
                lvmax = Convert.ToInt32(c0.Nodes["lvmax"]?.Value ?? 0);
            }
            if (check?.Nodes["1"] is Wz_Node c1)
            {
                check1Npc = Convert.ToInt32(c1.Nodes["npc"]?.Value ?? 0);
                foreach (var kind in new[] { "mob", "item" })
                {
                    var group = c1.Nodes[kind];
                    if (group == null) continue;
                    foreach (Wz_Node sub in group.Nodes)
                    {
                        check1Items.Add(new {
                            key = kind + sub.Text,
                            id = Convert.ToInt32(sub.Nodes["id"]?.Value ?? 0),
                            count = Convert.ToInt32(sub.Nodes["count"]?.Value ?? 0),
                        });
                    }
                }
                var ie = c1.Nodes["infoex"];
                if (ie != null)
                {
                    var seen = new HashSet<string>();
                    foreach (Wz_Node sub in ie.Nodes)
                    {
                        var v = sub.Nodes["exVariable"]?.Value?.ToString() ?? "";
                        if (!seen.Add(v)) continue;
                        var cond = Convert.ToInt32(sub.Nodes["cond"]?.Value ?? 0);
                        infoex.Add(new object[] { v, cond > 0 });
                    }
                }
            }
            if (recommendNode != null)
            {
                lvmin = Convert.ToInt32(recommendNode.Nodes["lvmin"]?.Value ?? 0);
                var lvmaxNode = recommendNode.Nodes["lvmax"];
                if (lvmaxNode != null)
                {
                    lvmax = Convert.ToInt32(lvmaxNode.Value);
                    lvLimit = true;
                }
            }
            bool recommendExcept = recommendNode?.Nodes["except"] != null;
            var reward = WzComparerR2.CharaSim.QuestReward.CreateFromAct1Node(node.Nodes["Act"]?.Nodes["1"]);
            rows.Add(new {
                id,
                name,
                desc,
                blocked,
                lvmin,
                lvmax,
                lv_limit = lvLimit,
                recommend_except = recommendExcept,
                demand_base = demandBase,
                check1_npc = check1Npc,
                check1_items = check1Items,
                check1_infoex = infoex,
                act1_reward = new {
                    exp = reward.Exp,
                    meso = reward.Meso,
                    pop = reward.Pop,
                    pet_tameness = reward.PetTameness,
                    attr_exps = reward.AttrExps.Select(kv => new object[] { kv.Key, kv.Value }).ToList(),
                    items = reward.Items.Select(i => new { id = i.ID, count = i.Count, prob = i.Prob, job = i.Job, gender = i.Gender }).ToList(),
                    sps = reward.Sps.Select(s => new { sp = s.SP, jobs = s.Jobs }).ToList(),
                },
            });
        }
        rows.Sort((a, b) => ((int)a.GetType().GetProperty("id").GetValue(a))
            .CompareTo((int)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void PluginFiles(string dir)
    {
        var files = Directory.Exists(dir)
            ? Directory.GetFiles(dir, "WzComparerR2.*.dll", SearchOption.AllDirectories)
                .Select(Path.GetFileName).OrderBy(x => x, StringComparer.Ordinal).ToList()
            : new List<string>();
        Console.WriteLine(JsonSerializer.Serialize(files, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemExtras(string dataRoot, string dir)
    {
        var rows = new List<object>();
        foreach (var file in Directory.GetFiles(Path.Combine(dataRoot, "Item", dir), "*.wz").OrderBy(x => x))
        {
            var st = new Wz_Structure(); st.Load(file);
            foreach (Wz_Node n in st.WzNode.Nodes)
            {
                if (n.Value is not Wz_Image img) continue;
                if (!img.TryExtract()) continue;
                foreach (Wz_Node item in img.Node.Nodes)
                {
                    var spec = item.Nodes["spec"];
                    if (spec == null) continue;
                    long? validDay = spec.Nodes["recipeValidDay"]?.Value is object v1 ? Convert.ToInt64(v1) : null;
                    long? useCount = spec.Nodes["recipeUseCount"]?.Value is object v2 ? Convert.ToInt64(v2) : null;
                    var lines = new List<string>();
                    if (validDay > 0) lines.Add($"( 제작 가능 기간 : {validDay}일 )");
                    if (useCount > 0) lines.Add($"( 제작 가능 횟수 : {useCount}회 )");
                    if (lines.Count == 0) continue;
                    rows.Add(new { id = item.Text, valid_day = validDay, use_count = useCount, lines });
                }
            }
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemCashLines(string dataRoot, string dir)
    {
        var rows = new List<object>();
        foreach (var file in Directory.GetFiles(Path.Combine(dataRoot, "Item", dir), "*.wz").OrderBy(x => x))
        {
            var st = new Wz_Structure(); st.Load(file);
            foreach (Wz_Node n in st.WzNode.Nodes)
            {
                if (n.Value is not Wz_Image img) continue;
                if (!img.TryExtract()) continue;
                foreach (Wz_Node item in img.Node.Nodes)
                {
                    if (!int.TryParse(item.Text, out int id)) continue;
                    var info = item.Nodes["info"] ?? item;
                    long Get(string key) => info.Nodes[key]?.Value is object v ? Convert.ToInt64(v) : 0;
                    var lines = new List<string>();
                    if (id / 1000 == 5533)
                        lines.Add("\n\n#c더블 클릭 시 미리보기에서 상자 속 아이템들을 3초마다 차례로 확인할 수 있습니다.\n\n캐시 보관함에서 더블 클릭하여 사용 가능하며, 상자는 교환할 수 없습니다.\n상자에서 획득한 보상품은 타인과 교환할 수 없습니다.#");
                    if (Get("cash") != 0)
                    {
                        if (Get("noMoveToLocker") > 0)
                            lines.Add("\n#c캐시 보관함으로 이동시킬 수 없는 아이템입니다.#");
                        if (Get("onlyCash") > 0)
                            lines.Add("\n#c넥슨캐시로만 구매할 수 있습니다.#");
                        if (Get("cashTradeBlock") > 0)
                            lines.Add("\n#c이 아이템은 넥슨캐시로 구매 후에도 타인과의 교환이 제한됩니다.#");
                        else if (Get("tradeBlock") == 0
                            && !(id / 10000 == 501 || id / 10000 == 502 || id / 10000 == 516
                                 || id / 1000 == 5157 || id / 1000 == 5158))
                            lines.Add("\n#c넥슨캐시로 구매하면 사용 전 1회에 한해 타인과 교환 할 수 있습니다.#");
                    }
                    if (Get("flatRate") > 0) lines.Add("\n#c기간 정액제 아이템입니다.#");
                    if (Get("noScroll") > 0) lines.Add("#c펫 스킬 주문서와 펫작명하기를 사용할 수 없습니다.#");
                    if (Get("noRevive") > 0) lines.Add("#c생명의 물을 사용할 수 없습니다.#");
                    if (lines.Count == 0) continue;
                    rows.Add(new { id = item.Text, lines });
                }
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
        else if (args[0] == "npcs") NpcGrid(args[1], args[2]);
        else if (args[0] == "reactors") ReactorGrid(args[1]);
        else if (args[0] == "music") MusicGrid(args[1]);
        else if (args[0] == "damageskins") DamageSkinGrid(args[1], args[2]);
        else if (args[0] == "morphs") MorphGrid(args[1], args[2], args[3]);
        else if (args[0] == "tamingmobs") TamingMobGrid(args[1], args[2]);
        else if (args[0] == "characters") CharacterGrid(args[1], args[2]);
        else if (args[0] == "skillsummary") SkillSummary(args[1], int.Parse(args[2]));
        else if (args[0] == "itemtooltip") ItemTooltip(args[1], args[2], args[3], args[4]);
        else if (args[0] == "itemtab") ItemTab(args[1], args[2], args[3]);
        else if (args[0] == "itemtooltips") ItemTooltips(args[1], args[2], args[3]);
        else if (args[0] == "gearprops") GearProps(args[1]);
        else if (args[0] == "openapi") OpenApi(args[1]);
        else if (args[0] == "networkrc4") NetworkRc4(args[1], args[2], args[3]);
        else if (args[0] == "jobnames") JobNames(int.Parse(args[1]));
        else if (args[0] == "corespec") CoreSpecs();
        else if (args[0] == "fifthjobnames") FifthJobNames();
        else if (args[0] == "achievements") AchievementTooltip(args[1]);
        else if (args[0] == "worldarchive") WorldArchive(args[1], args[2]);
        else if (args[0] == "findwz") FindWz(args, 1);
        else if (args[0] == "netpack") NetPack(args[1]);
        else if (args[0] == "charatooltips") CharaTooltips(args[1], args[2], args[3], args[4]);
        else if (args[0] == "damageskinmodel") DamageSkinModel(args[1], args[2]);
        else if (args[0] == "avatarstructure") AvatarStructure(int.Parse(args[1]));
        else if (args[0] == "avatarunpack") AvatarUnpack(int.Parse(args[1]), args[2]);
        else if (args[0] == "maptooltips") MapTooltips(args[1], args[2]);
        else if (args[0] == "questtooltips") QuestTooltips(args[1]);
        else if (args[0] == "pluginfiles") PluginFiles(args[1]);
        else if (args[0] == "itemextras") ItemExtras(args[1], args[2]);
        else if (args[0] == "itemcashlines") ItemCashLines(args[1], args[2]);
        else if (args[0] == "summaryprobe")
        {
            var r = new NoSkillResolver();
            foreach (var text in new[] { "#[400011003]x%", "#c강조#됩니다", "#t2초", "#cooltime초" })
                Console.WriteLine(text + " => " + WzComparerR2.CharaSim.SummaryParser.GetSkillSummary(
                    text, 1, r, WzComparerR2.CharaSim.SummaryParams.Default));
        }
        else if (args[0] == "set") SetTooltip(args[1], int.Parse(args[2]));
        else if (args[0] == "potential") PotentialTooltip(args[1], int.Parse(args[2]), int.Parse(args[3]));
        else Real(args[1]);
    }
}
