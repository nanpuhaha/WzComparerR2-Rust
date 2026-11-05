using System.Text.Json;
using WzComparerR2.WzLib;

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
    static void Main(string[] args)
    {
        if (args[0] == "fixture") Fixture(args[1]);
        else if (args[0] == "mapnodes") MapNodes(args[1], args[2]);
        else Real(args[1]);
    }
}
