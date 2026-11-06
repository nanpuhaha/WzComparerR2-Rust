using System.Text.Json;
using System.Security.Cryptography;
using WzComparerR2.WzLib;
using WzComparerR2.Common;
using WzComparerR2.CharaSim;
using System.Globalization;
using System.Reflection;
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
    static void ItemTextExtras(string dataRoot, string dir, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, null);
        string NameOf(int id)
        {
            linker.StringItem.TryGetValue(id, out var sr);
            if (sr == null) linker.StringEqp.TryGetValue(id, out sr);
            return string.IsNullOrEmpty(sr?.Name) ? null : sr.Name;
        }
        string ReplaceDescTags(string text)
        {
            return System.Text.RegularExpressions.Regex.Replace(text, @"#(t)\s*(\d{1,9}).*?#", m =>
            {
                if (!int.TryParse(m.Groups[2].Value, out int id)) id = -1;
                return NameOf(id) ?? id.ToString();
            });
        }
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
                    linker.StringItem.TryGetValue(id, out var sr);
                    var raw = sr?["desc_leftalign"];
                    var replaced = string.IsNullOrEmpty(raw) ? null : ReplaceDescTags(raw);
                    var info = item.Nodes["info"] ?? item;
                    long point = info.Nodes["pointCost"]?.Value is object pv ? Convert.ToInt64(pv) : 0;
                    var pointLine = point > 0 ? $"· {point} 포인트" : null;
                    var cant = info.Nodes["cantAccountSharable"]?.Nodes["tooltip"]?.Value?.ToString();
                    if (string.IsNullOrEmpty(raw) && pointLine == null && string.IsNullOrEmpty(cant)) continue;
                    rows.Add(new {
                        id,
                        desc_leftalign = string.IsNullOrEmpty(raw) ? null : raw,
                        desc_leftalign_replaced = replaced,
                        point_cost = pointLine,
                        cant_account_sharable = string.IsNullOrEmpty(cant) ? null : cant,
                    });
                }
            }
        }
        rows.Sort((a, b) => ((int)a.GetType().GetProperty("id").GetValue(a))
            .CompareTo((int)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemIncline(string dataRoot, string dir)
    {
        var rows = new List<object>();
        var types = new (string key, string label)[]
        {
            ("charismaEXP", "카리스마"), ("insightEXP", "통찰력"), ("willEXP", "의지"),
            ("craftEXP", "손재주"), ("senseEXP", "감성"), ("charmEXP", "매력"),
        };
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
                    string incline = null;
                    var joined = "";
                    foreach (var (key, label) in types)
                    {
                        var value = Get(key);
                        if (value > 0) joined += $", {label} {value}";
                    }
                    if (joined.Length > 0)
                        incline = "#c장착 시 1회에 한해 " + joined.Substring(2) + "의 경험치를 얻을 수 있습니다.(일일제한, 최대치 초과 시 제외)#";
                    string trade = null;
                    var tradeValue = Get("tradeAvailable");
                    if (tradeValue == 1)
                        trade = "#c" + "#c카르마의 가위 또는 실버 카르마의 가위를 사용하면 1회 교환이 가능하게 할 수 있습니다.#" + "#";
                    else if (tradeValue == 2)
                        trade = "#c" + "#c플래티넘 카르마의 가위를 사용하면 1회 교환이 가능하게 할 수 있습니다.#" + "#";
                    if (incline == null && trade == null) continue;
                    rows.Add(new { id, incline, trade_available = trade });
                }
            }
        }
        rows.Sort((a, b) => ((int)a.GetType().GetProperty("id").GetValue(a))
            .CompareTo((int)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void PluginEntries(string dllPath)
    {
        var asm = System.Reflection.Assembly.LoadFrom(dllPath);
        string Attr<T>() where T : Attribute
        {
            var a = (T)Attribute.GetCustomAttribute(asm, typeof(T));
            return a switch
            {
                AssemblyTitleAttribute t => t.Title,
                AssemblyCompanyAttribute c => c.Company,
                AssemblyInformationalVersionAttribute i => i.InformationalVersion,
                AssemblyFileVersionAttribute f => f.Version,
                _ => null,
            };
        }
        var baseType = typeof(WzComparerR2.PluginBase.PluginEntry);
        var types = asm.GetExportedTypes()
            .Where(t => t.IsSubclassOf(baseType) && !t.IsAbstract)
            .Select(t => t.FullName)
            .OrderBy(x => x, StringComparer.Ordinal)
            .ToList();
        Console.WriteLine(JsonSerializer.Serialize(new {
            name = asm.GetName().Name,
            version = asm.GetName().Version?.ToString(),
            title = Attr<AssemblyTitleAttribute>(),
            company = Attr<AssemblyCompanyAttribute>(),
            informational = Attr<AssemblyInformationalVersionAttribute>(),
            file_version = Attr<AssemblyFileVersionAttribute>(),
            plugin_types = types,
        }, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemRecipeReq(string dataRoot, string dir, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, null);
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
                    var spec = item.Nodes["spec"];
                    var recipeNode = spec?.Nodes["recipe"];
                    if (recipeNode == null) continue;
                    var recipes = new List<long>();
                    if (recipeNode.Nodes.Count == 0)
                    {
                        if (recipeNode.Value != null) recipes.Add(Convert.ToInt64(recipeNode.Value));
                    }
                    else
                    {
                        foreach (Wz_Node r in recipeNode.Nodes)
                            if (r.Value != null) recipes.Add(Convert.ToInt64(r.Value));
                    }
                    if (recipes.Count == 0) continue;
                    long? reqSkill = spec.Nodes["reqSkill"]?.Value is object v1 ? Convert.ToInt64(v1) : null;
                    long? reqLevel = spec.Nodes["reqSkillLevel"]?.Value is object v2 ? Convert.ToInt64(v2) : null;
                    var effectiveSkill = reqSkill ?? (recipes[0] / 10000 * 10000);
                    linker.StringSkill.TryGetValue((int)effectiveSkill, out var skr);
                    var name = string.IsNullOrEmpty(skr?.Name) ? "(null)" : skr.Name;
                    if (name == "장비제작") name = "장비 제작";
                    else if (name == "장신구제작") name = "장신구 제작";
                    var lines = new List<string> { "< 사용 제한조건 >", $"· {name} {reqLevel ?? 1}레벨 이상" };
                    rows.Add(new {
                        id,
                        recipes,
                        req_skill = reqSkill,
                        req_skill_level = reqLevel,
                        skill_name = skr?.Name,
                        lines,
                    });
                }
            }
        }
        rows.Sort((a, b) => ((int)a.GetType().GetProperty("id").GetValue(a))
            .CompareTo((int)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemExpire(string dataRoot, string dir)
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
                    var info = item.Nodes["info"];
                    if (info == null) continue;
                    var from = info.Nodes["consumableFrom"]?.Value?.ToString();
                    var end = info.Nodes["endUseDate"]?.Value?.ToString();
                    var lines = new List<string>();
                    // 원본은 길이 검증이 없어 12자 미만에서 예외가 난다.
                    // 결정적 비교를 위해 유효한 값만 처리한다.
                    if (!string.IsNullOrEmpty(from) && from.Length >= 12 && from.All(char.IsDigit))
                        lines.Add($"{Convert.ToInt32(from.Substring(0, 4))}년 {Convert.ToInt32(from.Substring(4, 2))}월 {Convert.ToInt32(from.Substring(6, 2))}일 {Convert.ToInt32(from.Substring(8, 2)):D2}시 {Convert.ToInt32(from.Substring(10, 2)):D2}분부터 사용가능");
                    if (!string.IsNullOrEmpty(end) && end.Length >= 12 && end.All(char.IsDigit))
                        lines.Add($"{Convert.ToInt32(end.Substring(0, 4))}년 {Convert.ToInt32(end.Substring(4, 2))}월 {Convert.ToInt32(end.Substring(6, 2))}일 {Convert.ToInt32(end.Substring(8, 2)):D2}시 {Convert.ToInt32(end.Substring(10, 2)):D2}분까지 사용가능");
                    if (lines.Count == 0) continue;
                    rows.Add(new { id, consumable_from = string.IsNullOrEmpty(from) ? null : from, end_use_date = string.IsNullOrEmpty(end) ? null : end, lines });
                }
            }
        }
        rows.Sort((a, b) => ((int)a.GetType().GetProperty("id").GetValue(a))
            .CompareTo((int)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    // 원본 `WzComparerR2.Animation.FrameAnimator` / `Controls.AnimationRecoder`는
    // SharpDX(Texture2D) 의존 때문에 macOS에서 어셈블리 타입 로드가 실패한다.
    // 아래는 원본 소스의 **메서드 본문을 그대로 옮긴** 참조 구현이다.
    // (docs/parity-inventory.md 참고)
    static class AnimReference
    {
        // FrameAnimator.CreateTimeline
        public static int[] CreateTimeline(IEnumerable<int> delays)
        {
            var timeLine = new List<int>() { 0 };
            foreach (var ms in delays)
            {
                timeLine.Add(timeLine[timeLine.Count - 1] + ms);
            }
            return timeLine.ToArray();
        }

        // FrameAnimator.GetProcessFromTimeline
        public static int GetProcessFromTimeline(int[] timeline, int timeOffset, out float progress)
        {
            int index = Array.BinarySearch(timeline, timeOffset);
            progress = 0;
            if (index < 0)
            {
                index = ~index - 1;
                progress = (float)(timeOffset - timeline[index]) / (timeline[index + 1] - timeline[index]);
            }
            return index;
        }

        // AnimationRecoder.GetGifTimeLine
        public static int[] GetGifTimeLine(
            IReadOnlyList<(int Length, bool Animated)> frames,
            int preferredFrameDelay,
            int? maxFrameDelay = null)
        {
            if (preferredFrameDelay <= 0)
            {
                preferredFrameDelay = 1;
            }
            if (maxFrameDelay != null && maxFrameDelay < preferredFrameDelay)
            {
                maxFrameDelay = preferredFrameDelay;
            }

            var timeline = new List<int>();
            int totalLength = 0;
            foreach (var frame in frames)
            {
                totalLength += frame.Length;
                if (frame.Animated)
                {
                    for (int ms = frame.Length; ms > 0;)
                    {
                        if (ms >= preferredFrameDelay)
                        {
                            timeline.Add(preferredFrameDelay);
                            ms -= preferredFrameDelay;
                        }
                        else
                        {
                            if (timeline.Count > 0)
                            {
                                timeline[timeline.Count - 1] += ms;
                            }
                            else
                            {
                                timeline.Add(ms);
                            }
                            ms = 0;
                        }
                    }
                }
                else
                {
                    if (maxFrameDelay != null)
                    {
                        for (int ms = frame.Length; ms > 0;)
                        {
                            if (ms >= maxFrameDelay.Value)
                            {
                                timeline.Add(maxFrameDelay.Value);
                                ms -= maxFrameDelay.Value;
                            }
                            else
                            {
                                timeline.Add(ms);
                                ms = 0;
                            }
                        }
                    }
                    else
                    {
                        timeline.Add(frame.Length);
                    }
                }
            }
            _ = totalLength;
            return timeline.ToArray();
        }

        // AnimationRecoder.GetMaxLength
        public static int GetMaxLength(IEnumerable<(int Start, int End)> itemTimes)
        {
            return itemTimes.Select(item => item.End).DefaultIfEmpty(0).Max();
        }

        // AnimationControl.AddItem / AnimationRecoder item bookkeeping
        public static (int Start, int End) AddItemTimes(int start, int length, int end)
        {
            return (0 + start, Math.Max(length, end) + start);
        }

        // AnimationControl.UpdateTimeOffset
        public static int UpdateTimeOffset(
            int ms, int lastUpdateMs, int timeOffsetMs, int maxLength, int frameInterval)
        {
            int currentTime = lastUpdateMs + timeOffsetMs;
            var round = (10 - currentTime % 10) % 10;
            double offset = ms + round + lastUpdateMs + timeOffsetMs;
            offset = Math.Max(0, Math.Min((maxLength - frameInterval) / 10 * 10, offset));
            return (int)offset;
        }

        // AnimationControl.Update reset condition: curTime > maxLength - margin
        public static bool ShouldReset(int curTime, int maxLength, int margin)
        {
            return curTime > maxLength - margin;
        }

        // AnimationControl.GlobalScale (MathHelper.Clamp)
        public static float ClampScale(float value)
        {
            return Math.Min(10f, Math.Max(0.1f, value));
        }
    }

    static void AnimTimeline()
    {
        var delaySets = new List<int[]>
        {
            Array.Empty<int>(),
            new[] { 1 },
            new[] { 90 },
            new[] { 30, 30, 30 },
            new[] { 100, 1, 100 },
            new[] { 255, 0, 7 },
            new[] { 10, 20, 30, 40, 50 },
        };
        var timelines = new List<object>();
        foreach (var delays in delaySets)
        {
            timelines.Add(new { delays, timeline = AnimReference.CreateTimeline(delays) });
        }

        var probes = new List<object>();
        foreach (var delays in delaySets)
        {
            var arr = AnimReference.CreateTimeline(delays);
            if (arr.Length < 2) continue;
            int total = arr[arr.Length - 1];
            foreach (var t in new[] { 0, 1, total / 2, Math.Max(0, total - 1), total })
            {
                var idx = AnimReference.GetProcessFromTimeline(arr, t, out float progress);
                probes.Add(new { delays, t, index = idx, progress });
            }
        }

        var frameSets = new (int[] Len, bool[] Ani)[]
        {
            (new[] { 90 }, new[] { false }),
            (new[] { 90 }, new[] { true }),
            (new[] { 30, 30, 30 }, new[] { true, true, true }),
            (new[] { 100, 1, 100 }, new[] { true, true, true }),
            (new[] { 255, 5, 7 }, new[] { false, true, false }),
            (new[] { 10, 20, 30, 40, 50 }, new[] { true, false, true, false, true }),
        };
        var gifCases = new List<object>();
        foreach (var (len, ani) in frameSets)
        {
            var frames = len.Select((l, i) => (l, ani[i])).ToList();
            foreach (var preferred in new[] { 0, 1, 20, 90, 200 })
            {
                foreach (var max in new int?[] { null, 30, 300 })
                {
                    gifCases.Add(new
                    {
                        len,
                        ani,
                        preferred,
                        max,
                        timeline = AnimReference.GetGifTimeLine(frames, preferred, max),
                    });
                }
            }
        }

        var itemTimeCases = new List<object>();
        foreach (var (start, length, end) in new (int, int, int)[]
        {
            (0, 0, 0), (0, 90, 0), (0, 0, 120), (50, 30, 0), (50, 30, 200), (-5, 10, 0),
        })
        {
            var pair = AnimReference.AddItemTimes(start, length, end);
            itemTimeCases.Add(new
            {
                start, length, end,
                pair = new { start = pair.Start, end = pair.End },
            });
        }
        var maxLengthCases = new[]
        {
            Array.Empty<(int, int)>(),
            new[] { (0, 90) },
            new[] { (0, 90), (100, 250) },
            new[] { (5, 5), (0, 0) },
        }.Select(t => new
        {
            pairs = t.Select(p => new { start = p.Item1, end = p.Item2 }).ToList(),
            max = AnimReference.GetMaxLength(t),
        }).ToList();

        var offsetCases = new List<object>();
        foreach (var (ms, last, off, maxLen, interval) in new (int, int, int, int, int)[]
        {
            (10, 0, 0, 1000, 10), (5, 123, 7, 1000, 10), (-100, 50, 0, 1000, 10),
            (1000, 0, 0, 500, 10), (33, 1000, 500, 1200, 10),
        })
        {
            offsetCases.Add(new
            {
                ms, last, off, max_len = maxLen, interval,
                value = AnimReference.UpdateTimeOffset(ms, last, off, maxLen, interval),
            });
        }

        var resetCases = new (int Cur, int Max, int Margin)[]
        {
            (0, 100, 10), (90, 100, 10), (91, 100, 10), (100, 100, 10), (5, 0, 10),
        };
        var scaleCases = new[] { -1f, 0f, 0.05f, 0.1f, 0.5f, 1f, 10f, 42f }
            .Select(v => new { input = v, output = AnimReference.ClampScale(v) }).ToList();

        Console.WriteLine(JsonSerializer.Serialize(new
        {
            timelines,
            probes,
            gif_cases = gifCases,
            item_time_cases = itemTimeCases,
            max_length_cases = maxLengthCases,
            time_offset_cases = offsetCases,
            reset_cases = resetCases.Select(c => new
            {
                cur = c.Cur, max = c.Max, margin = c.Margin,
                reset = AnimReference.ShouldReset(c.Cur, c.Max, c.Margin),
            }),
            scale_cases = scaleCases,
        }, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void MapPart(string dataRoot, int part, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, null);

        var parts = new string[][]
        {
            new[] { "Map0", "Map1", "Map2", "Map3" },
            new[] { "Map4", "Map5", "Map6", "Map7", "Map8" },
            new[] { "Map9" },
        };
        // own minimap + link per map id, across all partitions (원본은 Map/Map 전체를 연다)
        var entries = new Dictionary<int, (string dir, bool own, int? link)>();
        foreach (var dir in parts.SelectMany(x => x))
        {
            var dirPath = Path.Combine(dataRoot, "Map", "Map", dir);
            if (!Directory.Exists(dirPath)) continue;
            foreach (var file in Directory.GetFiles(dirPath, "*.wz").OrderBy(x => x))
            {
                var st = new Wz_Structure(); st.Load(file);
                foreach (Wz_Node n in st.WzNode.Nodes)
                {
                    if (n.Value is not Wz_Image img) continue;
                    var idText = n.Text.Replace(".img", "");
                    if (!int.TryParse(idText, out int id)) continue;
                    if (!img.TryExtract()) continue;
                    var own = img.Node.FindNodeByPath(@"miniMap\canvas") != null;
                    var linkNode = img.Node.FindNodeByPath(@"info\link");
                    int? link = linkNode?.Value is object v ? Convert.ToInt32(v) : null;
                    entries[id] = (dir, own, link);
                }
            }
        }

        var rows = new List<object>();
        foreach (var dir in parts[part - 1])
        {
            var dirPath = Path.Combine(dataRoot, "Map", "Map", dir);
            if (!Directory.Exists(dirPath)) continue;
            foreach (var file in Directory.GetFiles(dirPath, "*.wz").OrderBy(x => x))
            {
                var st = new Wz_Structure(); st.Load(file);
                foreach (Wz_Node n in st.WzNode.Nodes)
                {
                    if (n.Value is not Wz_Image img) continue;
                    var idText = n.Text.Replace(".img", "");
                    if (idText.Length == 0 || !idText.All(char.IsDigit)) continue;
                    if (!int.TryParse(idText, out int id)) continue;
                    if (!entries.TryGetValue(id, out var entry)) continue;
                    if (!img.TryExtract()) continue;
                    linker.StringMap.TryGetValue(id, out var sr);
                    string street = sr?.StreetName ?? "";
                    string mapName = sr?.MapName ?? "";
                    bool hasMini = entry.own;
                    if (!hasMini && entry.link != null)
                    {
                        var first = entry.link.Value.ToString();
                        var targetDir = "Map" + first.Substring(0, 1);
                        if (entries.TryGetValue(entry.link.Value, out var target))
                            hasMini = target.dir == targetDir && target.own;
                    }
                    rows.Add(new
                    {
                        id = idText,
                        street_name = street,
                        map_name = mapName,
                        has_minimap = hasMini,
                        link = entry.link,
                    });
                }
            }
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void MobPart(string dataRoot, int part, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, null);

        // Mob/<id>.img 존재 여부 + 필드 (원본은 열린 Mob 구조 전체를 검색)
        var found = new Dictionary<int, Wz_Node>();
        var files = Directory.GetFiles(Path.Combine(dataRoot, "Mob"), "*.wz", SearchOption.AllDirectories)
            .OrderBy(x => x, StringComparer.Ordinal).ToList();
        foreach (var file in files)
        {
            var st = new Wz_Structure(); st.Load(file);
            foreach (Wz_Node n in st.WzNode.Nodes)
            {
                if (n.Value is not Wz_Image img) continue;
                if (!int.TryParse(n.Text.Replace(".img", ""), out int id)) continue;
                if (found.ContainsKey(id)) continue;
                if (!img.TryExtract()) continue;
                found[id] = img.Node;
            }
        }

        bool Accept(int id)
        {
            int first = (id / 1000000) % 10;
            return part switch
            {
                1 => first != 8 && first != 9,
                2 => first == 8,
                3 => first == 9,
                _ => false,
            };
        }

        var rows = new List<object>();
        foreach (var kv in linker.StringMob)
        {
            int id = kv.Key;
            if (!found.TryGetValue(id, out var node)) continue;
            if (!Accept(id)) continue;
            rows.Add(new
            {
                id = id.ToString("D7"),
                name = kv.Value.Name ?? "",
                has_stand = node.Nodes["stand"] != null,
                has_fly = node.Nodes["fly"] != null,
                has_move = node.Nodes["move"] != null,
                has_hit = node.Nodes["hit1"] != null || node.Nodes["hit"] != null,
                has_die = node.Nodes["die1"] != null || node.Nodes["die"] != null,
                has_thumbnail = node.FindNodeByPath(@"info\thumbnail") != null,
                link = node.FindNodeByPath(@"info\link")?.Value is object lv ? Convert.ToInt32(lv) : (int?)null,
            });
        }
        rows.Sort((a, b) => string.CompareOrdinal(
            (string)a.GetType().GetProperty("id").GetValue(a),
            (string)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void SkillTooltips(string dataRoot, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, null);

        var files = Directory.GetFiles(Path.Combine(dataRoot, "Skill"), "*.wz", SearchOption.AllDirectories)
            .OrderBy(x => x, StringComparer.Ordinal).ToList();
        var seen = new HashSet<int>();
        var rows = new List<object>();
        foreach (var file in files)
        {
            var st = new Wz_Structure(); st.Load(file);
            foreach (Wz_Node imgNode in st.WzNode.Nodes)
            {
                if (imgNode.Value is not Wz_Image img) continue;
                if (!img.TryExtract()) continue;
                var skillRoot = img.Node.Nodes["skill"];
                if (skillRoot == null) continue;
                foreach (Wz_Node skill in skillRoot.Nodes)
                {
                    if (!int.TryParse(skill.Text, out int id)) continue;
                    if (!seen.Add(id)) continue;

                    var levelNode = skill.Nodes["level"];
                    int levelCount = 0;
                    if (levelNode != null)
                    {
                        while (levelNode.Nodes[(levelCount + 1).ToString()] != null) levelCount++;
                    }
                    bool preBB = levelCount > 0;
                    bool hyper = Convert.ToInt32(skill.Nodes["hyperStat"]?.Value ?? 0) != 0;
                    int maxLevel;
                    if (preBB)
                    {
                        maxLevel = levelCount;
                    }
                    else
                    {
                        var maxNode = skill.Nodes["common"]?.Nodes["maxLevel"];
                        maxLevel = maxNode?.Value is object mv ? Convert.ToInt32(mv) : 0;
                    }

                    int reqLevel = skill.Nodes["reqLev"]?.Value is object rv ? Convert.ToInt32(rv) : 0;
                    int reqAmount = 0;
                    var reqSkills = new List<object>();
                    var reqNode = skill.Nodes["req"];
                    if (reqNode != null)
                    {
                        foreach (Wz_Node entry in reqNode.Nodes)
                        {
                            if (entry.Text == "level") reqLevel = Convert.ToInt32(entry.Value);
                            else if (entry.Text == "reqAmount") reqAmount = Convert.ToInt32(entry.Value);
                            else if (int.TryParse(entry.Text, out int reqSkill))
                                reqSkills.Add(new { id = reqSkill, level = Convert.ToInt32(entry.Value) });
                        }
                    }

                    linker.StringSkill.TryGetValue(id, out var sr);
                    var name = string.IsNullOrEmpty(sr?.Name) ? "(null)" : sr.Name;
                    var lines = new List<string> { name };
                    if (hyper) lines.Add($"[최대 레벨 : {maxLevel}]");
                    else if (!preBB) lines.Add($"[마스터 레벨 : {maxLevel}]");

                    rows.Add(new
                    {
                        id,
                        name,
                        max_level = maxLevel,
                        pre_bb = preBB,
                        hyper_stat = hyper,
                        req_level = reqLevel,
                        req_amount = reqAmount,
                        req_skills = reqSkills,
                        desc = sr?.Desc,
                        lines,
                    });
                }
            }
        }
        rows.Sort((a, b) => ((int)a.GetType().GetProperty("id").GetValue(a))
            .CompareTo((int)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    // 원본 `LuaSandbox.InitLuaEnv`의 package.path 구성 부분을 그대로 옮긴
    // 참조 구현. 원본 타입은 NLua(Lua 5.1 네이티브) + WinForms
    // (`Application.StartupPath`)에 묶여 macOS에서 로드할 수 없다.
    static string[] LuaPackagePath(string scriptBaseDir, string pluginBaseDir, string wcR2Folder, string cwd)
    {
        var packagePath = new List<string>(16);
        foreach (var baseDir in new[] { scriptBaseDir, pluginBaseDir, wcR2Folder })
        {
            if (!string.IsNullOrEmpty(baseDir))
            {
                packagePath.Add(Path.Combine(baseDir, "?.lua"));
                packagePath.Add(Path.Combine(baseDir, "?", "init.lua"));
                packagePath.Add(Path.Combine(baseDir, "lua", "?.lua"));
                packagePath.Add(Path.Combine(baseDir, "lua", "?", "init.lua"));
            }
        }
        packagePath.Add(Path.Combine(cwd, "?.lua"));
        packagePath.Add(Path.Combine(cwd, "?", "init.lua"));
        return packagePath.Distinct().ToArray();
    }
    static void LuaPath(string scriptBaseDir, string pluginBaseDir, string wcR2Folder, string cwd)
    {
        var entries = LuaPackagePath(scriptBaseDir, pluginBaseDir, wcR2Folder, cwd);
        Console.WriteLine(JsonSerializer.Serialize(new
        {
            entries,
            joined = string.Join(";", entries),
        }, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void PluginActivate(string dllPath, string pluginBasePath)
    {
        var pluginAsm = System.Reflection.Assembly.LoadFrom(pluginBasePath);
        var contextType = pluginAsm.GetType("WzComparerR2.PluginBase.PluginContext")!;
        var entryType = pluginAsm.GetType("WzComparerR2.PluginBase.PluginEntry")!;
        object context = null;
        string contextError = null;
        try
        {
            context = Activator.CreateInstance(contextType,
                System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic,
                null, new object[] { null }, null);
        }
        catch (Exception ex)
        {
            contextError = ex.GetType().Name + ": " + ex.Message;
        }

        var asm = System.Reflection.Assembly.LoadFrom(dllPath);
        var rows = new List<object>();
        foreach (var type in asm.GetExportedTypes()
            .Where(t => t.IsSubclassOf(entryType) && !t.IsAbstract)
            .OrderBy(t => t.FullName, StringComparer.Ordinal))
        {
            string error = null;
            string name = null, author = null, version = null, fileVersion = null;
            try
            {
                var instance = Activator.CreateInstance(type, new[] { context })!;
                var entry = (WzComparerR2.PluginBase.PluginEntry)instance;
                name = entry.Name;
                author = entry.Author;
                version = entry.Version;
                fileVersion = entry.FileVersion;
            }
            catch (Exception ex)
            {
                error = ex.GetType().Name + ": " + ex.Message;
            }
            rows.Add(new
            {
                type = type.FullName,
                name,
                author,
                version,
                file_version = fileVersion,
                error,
            });
        }
        Console.WriteLine(JsonSerializer.Serialize(new
        {
            context_created = context != null,
            context_error = contextError,
            entries = rows,
        }, new JsonSerializerOptions { WriteIndented = true }));
    }
    static void ItemCosmeticEulo(string dataRoot, string stringWzPath)
    {
        var str = new Wz_Structure(); str.Load(stringWzPath);
        var linker = new StringLinker();
        linker.Load(str.wz_files[0], null, null);

        var rows = new List<object>();
        foreach (var dir in new[] { "Cash", "Consume", "Install", "Etc", "Special", "Pet" })
        {
            var dirPath = Path.Combine(dataRoot, "Item", dir);
            if (!Directory.Exists(dirPath)) continue;
            foreach (var file in Directory.GetFiles(dirPath, "*.wz").OrderBy(x => x, StringComparer.Ordinal))
            {
                var st = new Wz_Structure(); st.Load(file);
                foreach (Wz_Node n in st.WzNode.Nodes)
                {
                    if (n.Value is not Wz_Image img) continue;
                    if (!img.TryExtract()) continue;
                    foreach (Wz_Node item in img.Node.Nodes)
                    {
                        if (!int.TryParse(item.Text, out int id)) continue;
                        var cosmeticNode = item.FindNodeByPath(@"spec\cosmetic");
                        if (cosmeticNode?.Value == null) continue;
                        int cosmeticId = Convert.ToInt32(cosmeticNode.Value);
                        linker.StringItem.TryGetValue(id, out var sr);
                        var desc = sr?.Desc;
                        if (desc == null || !desc.Contains("#cosmetic_EULO#")) continue;

                        int gender = WzComparerR2.CharaSim.Gear.GetGender(cosmeticId);
                        linker.StringEqp.TryGetValue(cosmeticId, out var eqp);
                        var name = $"#c{eqp?.Name ?? "(null)"}{(gender == 0 ? "(남)" : (gender == 1 ? "(여)" : ""))}#";
                        int last = (name.LastOrDefault(c => c >= '가' && c <= '힣') - '가') % 28;
                        name += ((last == 0 || last == 8 ? "" : "으") + "로");
                        // AvatarCanvas.HairColor 8종
                        foreach (var color in new[] { "검은색", "빨간색", "주황색", "노란색",
                                                     "초록색", "파란색", "보라색", "갈색" })
                        {
                            name = name.Replace($"{color} ", "");
                        }
                        rows.Add(new
                        {
                            id,
                            cosmetic_id = cosmeticId,
                            gender,
                            eqp_name = eqp?.Name,
                            replacement = name,
                            replaced = desc.Replace("#cosmetic_EULO#", name),
                        });
                    }
                }
            }
        }
        rows.Sort((a, b) => ((int)a.GetType().GetProperty("id").GetValue(a))
            .CompareTo((int)b.GetType().GetProperty("id").GetValue(b)));
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    // 원본 `DataGrid.SaveBin`/`LoadBin`의 직렬화 루프를 그대로 옮기되,
    // WinForms `DataGridView` 대신 행/셀 값을 직접 쓴다. BinaryWriter/Reader는
    // 실제 .NET 구현을 쓴다. PNG는 `MemoryStream.GetBuffer()`(용량) 대신
    // 정확한 길이를 써서 플랫폼 의존 패딩을 제거한다(docs 참고).
    static void Db2Bin()
    {
        var rows = new List<(string id, byte[] png)>
        {
            ("가", new byte[] { 1, 2, 3 }),
            ("나", Enumerable.Repeat((byte)0xFF, 300).ToArray()),
            ("다", Array.Empty<byte>()),
        };
        var ms = new MemoryStream();
        var writer = new BinaryWriter(ms, System.Text.Encoding.UTF8, true);
        writer.Write(2);
        writer.Write(rows.Count);
        foreach (var (id, png) in rows)
        {
            writer.Write("str");
            writer.Write(id);
            writer.Write("Bitmap");
            writer.Write(png.Length);
            writer.Write(png);
        }
        writer.Flush();
        var bytes = ms.ToArray();

        var reader = new BinaryReader(new MemoryStream(bytes), System.Text.Encoding.UTF8, true);
        int n = reader.ReadInt32();
        int m = reader.ReadInt32();
        var parsed = new List<object>();
        for (int i = 0; i <= m - 2; i++)
        {
            var cells = new List<object>();
            for (int j = 0; j <= n - 1; j++)
            {
                if (reader.ReadString() == "str")
                    cells.Add(reader.ReadString());
                else
                {
                    var len = reader.ReadInt32();
                    cells.Add(Convert.ToHexString(reader.ReadBytes(len)).ToLowerInvariant());
                }
            }
            parsed.Add(cells);
        }
        Console.WriteLine(JsonSerializer.Serialize(new
        {
            columns = 2,
            declared_rows = rows.Count,
            bin_base64 = Convert.ToBase64String(bytes),
            parsed_rows = parsed,
            parsed_row_count = parsed.Count,
        }, new JsonSerializerOptions { WriteIndented = true }));
    }
    // 원본 `WzComparerR2.Text.TextRenderer<TFont>`를 **그대로 상속**해
    // 측정 함수만 결정적으로 바꾼다. 즉 레이아웃 루프는 실제 원본 코드다.
    sealed class TextLayoutProbe : WzComparerR2.Text.TextRenderer<string>
    {
        public List<(int start, int len, int x, int y)> Rects = new();
        public int EndY;
        public static int Measure(string s) => s.Length * 7;

        protected override void MeasureRuns(List<WzComparerR2.Text.Run> runs)
        {
            int x = 0;
            foreach (var run in runs)
            {
                if (run.IsBreakLine)
                {
                    run.X = x;
                    run.Length = 0;
                }
                else
                {
                    run.X = x;
                    run.Width = Measure(this.sb.ToString(run.StartIndex, run.Length));
                    x += run.Width;
                }
            }
        }

        protected override System.Drawing.Rectangle[] MeasureChars(int startIndex, int length)
        {
            var regions = new System.Drawing.Rectangle[length];
            int x = 0;
            for (int i = 0; i < length; i++)
            {
                int w = Measure(this.sb[startIndex + i].ToString());
                regions[i] = new System.Drawing.Rectangle(x, 0, w, 16);
                x += w;
            }
            return regions;
        }

        protected override void Flush(System.Text.StringBuilder sb, int startIndex, int length,
            int x, int y, string ColorID, string FontID, string ImageID, int imageWidth, int imageHeight)
        {
            Rects.Add((startIndex, length, x, y));
        }

        public void Run(string text, int width, int y, int height, WzComparerR2.Text.TextAlignment align)
        {
            Rects.Clear();
            int yy = y;
            this.WordWrapEnabled = false;
            this.DrawPlainText(text, "", width, ref yy, height, align);
            EndY = yy;
        }
    }

    static void TextLayout()
    {
        var cases = new (string text, int width, int y, int height, WzComparerR2.Text.TextAlignment align)[]
        {
            ("hello world", 50, 0, 16, WzComparerR2.Text.TextAlignment.Left),
            ("동해물과 백두산이 마르고 닳도록", 60, 0, 16, WzComparerR2.Text.TextAlignment.Left),
            ("abcdefghij", 21, 0, 16, WzComparerR2.Text.TextAlignment.Left),
            ("ab\ncd", 1000, 5, 16, WzComparerR2.Text.TextAlignment.Left),
            ("가나다라마바사아자차", 35, 3, 18, WzComparerR2.Text.TextAlignment.Center),
            ("가나다라마바사아자차", 35, 3, 18, WzComparerR2.Text.TextAlignment.Right),
            ("a,b,c,d,e", 20, 0, 16, WzComparerR2.Text.TextAlignment.Left),
            ("  spaced  ", 30, 0, 16, WzComparerR2.Text.TextAlignment.Left),
            ("", 40, 0, 16, WzComparerR2.Text.TextAlignment.Left),
            ("줄1\r\n줄2\r줄3\n줄4", 1000, 0, 16, WzComparerR2.Text.TextAlignment.Left),
            ("가나", 100, 0, 16, WzComparerR2.Text.TextAlignment.Center),
            ("가나", 100, 0, 16, WzComparerR2.Text.TextAlignment.Right),
        };
        var probe = new TextLayoutProbe();
        var rows = new List<object>();
        foreach (var (text, width, y, height, align) in cases)
        {
            probe.Run(text, width, y, height, align);
            rows.Add(new
            {
                text,
                width,
                start_y = y,
                line_height = height,
                alignment = align.ToString(),
                rects = probe.Rects.Select(r => new { start = r.start, len = r.len, x = r.x, y = r.y }).ToList(),
                end_y = probe.EndY,
            });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    // 원본 `GearGraphics.EnlargeBitmap`의 바이트 루프를 그대로 옮긴 참조 구현.
    // `Bitmap.LockBits`/`Marshal.Copy`는 32bpp에서 stride == width*4 이므로
    // 바이트 배열 연산과 동일하다.
    static byte[] EnlargeBitmapBytes(byte[] origin, int width, int height)
    {
        int stride = width * 4;
        var newByte = new byte[origin.Length * 4];
        var buffer = new byte[4];
        for (int i = 0; i < height; i++)
        {
            for (int j = 0; j < width; j++)
            {
                Array.Copy(origin, i * stride + j * 4, buffer, 0, 4);
                int dst = (2 * i * (width * 2) + 2 * j) * 4;
                Array.Copy(buffer, 0, newByte, dst, 4);
                Array.Copy(buffer, 0, newByte, dst + 4, 4);
            }
            int rowDst = 2 * i * (width * 2) * 4;
            Array.Copy(newByte, rowDst, newByte, rowDst + width * 2 * 4, width * 2 * 4);
        }
        return newByte;
    }

    static void IconBlock(string fixturePath)
    {
        using var doc = System.Text.Json.JsonDocument.Parse(File.ReadAllText(fixturePath));
        var canvasEl = doc.RootElement.GetProperty("canvas");
        int cw = canvasEl.GetProperty("width").GetInt32();
        int ch = canvasEl.GetProperty("height").GetInt32();
        int iconX = canvasEl.GetProperty("icon_x").GetInt32();
        int picH = canvasEl.GetProperty("pic_h").GetInt32();

        var rows = new List<object>();
        foreach (var icon in doc.RootElement.GetProperty("icons").EnumerateArray())
        {
            int w = icon.GetProperty("width").GetInt32();
            int h = icon.GetProperty("height").GetInt32();
            var rgba = Convert.FromBase64String(icon.GetProperty("rgba_b64").GetString()!);
            var enlarged = EnlargeBitmapBytes(rgba, w, h);

            var originEl = icon.GetProperty("origin");
            int ox = originEl[0].GetInt32();
            int oy = originEl[1].GetInt32();
            // 원본 ItemTooltipRender22 좌표식
            int dx = iconX + 6 + (1 - ox) * 2;
            int dy = picH + 6 + (33 - oy) * 2;

            var canvas = new byte[cw * ch * 4];
            int ew = w * 2, eh = h * 2;
            for (int y = 0; y < eh; y++)
            {
                int ty = dy + y;
                if (ty < 0 || ty >= ch) continue;
                for (int x = 0; x < ew; x++)
                {
                    int tx = dx + x;
                    if (tx < 0 || tx >= cw) continue;
                    int si = (y * ew + x) * 4;
                    if (enlarged[si + 3] != 255) continue; // 불투명 픽셀만
                    int di = (ty * cw + tx) * 4;
                    Array.Copy(enlarged, si, canvas, di, 4);
                }
            }

            using var sha = System.Security.Cryptography.SHA256.Create();
            rows.Add(new
            {
                path = icon.GetProperty("path").GetString(),
                dx,
                dy,
                enlarged_width = ew,
                enlarged_height = eh,
                canvas_width = cw,
                canvas_height = ch,
                out_len = canvas.Length,
                out_sha256 = Convert.ToHexString(sha.ComputeHash(canvas)).ToLowerInvariant(),
            });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }

    static void PixelOps(string fixturePath)
    {
        using var doc = System.Text.Json.JsonDocument.Parse(File.ReadAllText(fixturePath));
        var rows = new List<object>();
        foreach (var icon in doc.RootElement.GetProperty("icons").EnumerateArray())
        {
            int w = icon.GetProperty("width").GetInt32();
            int h = icon.GetProperty("height").GetInt32();
            var rgba = Convert.FromBase64String(icon.GetProperty("rgba_b64").GetString()!);
            var enlarged = EnlargeBitmapBytes(rgba, w, h);
            using var sha = System.Security.Cryptography.SHA256.Create();
            rows.Add(new
            {
                path = icon.GetProperty("path").GetString(),
                width = w,
                height = h,
                out_width = w * 2,
                out_height = h * 2,
                out_len = enlarged.Length,
                out_sha256 = Convert.ToHexString(sha.ComputeHash(enlarged)).ToLowerInvariant(),
            });
        }
        Console.WriteLine(JsonSerializer.Serialize(rows, new JsonSerializerOptions { WriteIndented = true }));
    }
    // `AnimationControl`의 히트테스트/드래그/휠 계산. WinForms 이벤트 대신
    // 좌표/델타를 받되, 반올림과 사각형 판정은 실제 .NET API를 쓴다.
    static void ControlsInput()
    {
        var items = new (int px, int py, int bx, int by, int bw, int bh)[]
        {
            (0, 0, 0, 0, 100, 50),
            (20, 10, -5, -5, 30, 30),
            (-3, 7, 2, 2, 10, 10),
        };
        var scales = new[] { 0.1f, 0.5f, 1.0f, 1.333f, 2.0f, 10.0f };
        var points = new[]
        {
            (0, 0), (5, 5), (9, 9), (10, 10), (11, 4), (20, 20), (-3, 7), (-4, 6), (25, 12), (24, 11),
        };

        System.Drawing.Rectangle Rect(int px, int py, int bx, int by, int bw, int bh, float scale) =>
            new System.Drawing.Rectangle(
                (int)Math.Round(px + bx * scale),
                (int)Math.Round(py + by * scale),
                (int)Math.Round(bw * scale),
                (int)Math.Round(bh * scale));

        var rects = new List<object>();
        foreach (var (px, py, bx, by, bw, bh) in items)
        {
            foreach (var scale in scales)
            {
                var r = Rect(px, py, bx, by, bw, bh, scale);
                rects.Add(new { px, py, bx, by, bw, bh, scale, x = r.X, y = r.Y, w = r.Width, h = r.Height });
            }
        }

        var hits = new List<object>();
        foreach (var scale in scales)
        {
            foreach (var (x, y) in points)
            {
                var found = new List<int>();
                for (int i = items.Length - 1; i >= 0; i--)
                {
                    var (px, py, bx, by, bw, bh) = items[i];
                    if (Rect(px, py, bx, by, bw, bh, scale).Contains(x, y)) found.Add(i);
                }
                hits.Add(new { scale, x, y, indices = found });
            }
        }

        // 0.5 경계(은행가 반올림) 케이스
        var ties = new List<object>();
        foreach (var (bx, scale) in new (int, float)[] { (3, 0.5f), (5, 0.5f), (7, 0.5f), (1, 1.5f), (3, 1.5f), (5, 1.5f) })
        {
            var r = Rect(0, 0, bx, 0, 0, 0, scale);
            ties.Add(new { bx, scale, x = r.X, y = r.Y });
        }

        var drags = new List<object>();
        foreach (var (mdx, mdy, sx, sy, cx, cy) in new (int, int, int, int, int, int)[]
        {
            (10, 20, 5, 6, 13, 29), (0, 0, 0, 0, 0, 0), (-5, 3, 10, 10, -5, 3), (100, 100, 0, 0, 99, 101),
        })
        {
            drags.Add(new { mouse_down = new[] { mdx, mdy }, start = new[] { sx, sy }, current = new[] { cx, cy },
                            position = new[] { cx - mdx + sx, cy - mdy + sy } });
        }

        var dragBoxes = new List<object>();
        foreach (var (dw, dh) in new (int, int)[] { (4, 4), (8, 8) })
        {
            foreach (var (cx, cy) in new[] { (10, 20), (13, 23), (14, 24), (5, 15) })
            {
                var box = new System.Drawing.Rectangle(new System.Drawing.Point(10, 20), new System.Drawing.Size(dw, dh));
                dragBoxes.Add(new { drag_size = new[] { dw, dh }, current = new[] { cx, cy }, inside = box.Contains(cx, cy) });
            }
        }

        static float Wheel(float oldScale, int delta)
        {
            const int WHEEL_DELTA = 120;
            float wheelTicks = delta / WHEEL_DELTA;
            float newScale = oldScale * (1 + 0.1f * wheelTicks);
            if (oldScale.CompareTo(1f) * newScale.CompareTo(1f) == -1) newScale = 1f;
            return Math.Min(10f, Math.Max(0.1f, newScale));
        }
        var wheels = new List<object>();
        foreach (var (oldScale, delta) in new (float, int)[]
        {
            (1.0f, 120), (1.0f, 240), (1.0f, 60), (1.0f, -120), (0.95f, -120), (0.95f, 120),
            (9.9f, 240), (0.1f, -240), (2.0f, 0),
        })
        {
            wheels.Add(new { old_scale = oldScale, delta, new_scale = Wheel(oldScale, delta) });
        }

        Console.WriteLine(JsonSerializer.Serialize(new { rects, hits, ties, drags, drag_boxes = dragBoxes, wheels },
            new JsonSerializerOptions { WriteIndented = true }));
    }
    // `LuaSandbox.InitLuaEnv`의 프렐류드를 **원문 그대로** NLua로 실행해
    // CLR 브리지(`luanet.import_type`/`get_method_bysig`) 동작을 기록한다.
    // WinForms(`Application.StartupPath`)와 `Entry` 어셈블리 경로 계산만 제외.
    const string LuaClrPrelude = @"
local t_IEnumerable = {}
t_IEnumerable.typeRef = luanet.import_type('System.Collections.IEnumerable')
t_IEnumerable.GetEnumerator = luanet.get_method_bysig(t_IEnumerable.typeRef, 'GetEnumerator')

local t_IEnumerator = {}
t_IEnumerator.typeRef = luanet.import_type('System.Collections.IEnumerator')
t_IEnumerator.MoveNext = luanet.get_method_bysig(t_IEnumerator.typeRef, 'MoveNext')
t_IEnumerator.get_Current = luanet.get_method_bysig(t_IEnumerator.typeRef, 'get_Current')

function each(userData)
  if type(userData) == 'userdata' then
    local i = 0;
    local ienum = t_IEnumerable.GetEnumerator(userData)
    return function()
      if ienum and t_IEnumerator.MoveNext(ienum) then
        i = i + 1
        return i, t_IEnumerator.get_Current(ienum)
      end
      return nil, nil
    end
  end
end
";

    static void LuaClr()
    {
        using var lua = new NLua.Lua();
        lua.State.Encoding = System.Text.Encoding.UTF8;
        lua.LoadCLRPackage();
        lua["env"] = new { tag = "host" };
        lua.DoString(LuaClrPrelude);

        var objects = new (string name, object value, string kind)[]
        {
            ("list_int", new List<int> { 10, 20, 30 }, "clr"),
            ("list_str", new List<string> { "가", "나" }, "clr"),
            ("array_int", new[] { 1, 2, 3 }, "clr"),
            ("empty_list", new List<int>(), "clr"),
            ("lua_table", null, "table"),
            ("nil_value", null, "nil"),
        };
        var rows = new List<object>();
        foreach (var (name, value, kind) in objects)
        {
            if (kind == "clr") lua[name] = value;
            var arg = kind switch
            {
                "clr" => name,
                "table" => "{1,2,3}",
                _ => "nil",
            };
            var count = lua.DoString($"return select('#', each({arg}))")[0]?.ToString() ?? "0";
            var res = lua.DoString($@"
local ok, acc = pcall(function()
  local out = ''
  for i, v in each({arg}) do out = out .. i .. '=' .. tostring(v) .. ';' end
  return out
end)
return tostring(ok) .. '|' .. tostring(acc)
");
            var parts = (res[0]?.ToString() ?? "false|").Split('|', 2);
            rows.Add(new
            {
                name,
                kind,
                iterator_count = count,
                ok = parts[0] == "true",
                value = parts.Length > 1 ? parts[1] : null,
            });
        }

        var envType = lua.DoString("return type(env)")[0]?.ToString();
        var envTag = lua.DoString("return env.tag")[0]?.ToString();
        Console.WriteLine(JsonSerializer.Serialize(new { cases = rows, env_type = envType, env_tag = envTag },
            new JsonSerializerOptions { WriteIndented = true }));
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
        else if (args[0] == "itemtextextras") ItemTextExtras(args[1], args[2], args[3]);
        else if (args[0] == "itemincline") ItemIncline(args[1], args[2]);
        else if (args[0] == "pluginentries") PluginEntries(args[1]);
        else if (args[0] == "itemrecipereq") ItemRecipeReq(args[1], args[2], args[3]);
        else if (args[0] == "itemexpire") ItemExpire(args[1], args[2]);
        else if (args[0] == "animtimeline") AnimTimeline();
        else if (args[0] == "mappart") MapPart(args[1], int.Parse(args[2]), args[3]);
        else if (args[0] == "mobpart") MobPart(args[1], int.Parse(args[2]), args[3]);
        else if (args[0] == "skilltooltips") SkillTooltips(args[1], args[2]);
        else if (args[0] == "luapath") LuaPath(args[1], args[2], args[3], args[4]);
        else if (args[0] == "pluginactivate") PluginActivate(args[1], args[2]);
        else if (args[0] == "itemcosmetic") ItemCosmeticEulo(args[1], args[2]);
        else if (args[0] == "db2bin") Db2Bin();
        else if (args[0] == "textlayout") TextLayout();
        else if (args[0] == "pixelops") PixelOps(args[1]);
        else if (args[0] == "iconblock") IconBlock(args[1]);
        else if (args[0] == "controlsinput") ControlsInput();
        else if (args[0] == "luaclr") LuaClr();
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
