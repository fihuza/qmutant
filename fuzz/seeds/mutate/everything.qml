pragma ComponentBehavior: Bound
import QtQuick
import "Model.js" as Model

Item {
    id: root

    property int count: 0
    property var names: ["a", "b"]
    readonly property bool busy: count >= 10 && label !== ""
    property string label: "idle"
    signal finished(string reason)

    width: busy ? 100 : 50
    opacity: -root.count < 0 ? 1 : 0.5

    function total(values) {
        let sum = 0;
        for (let i = 0; i < values.length; i++) {
            sum += values[i] * 2;
        }
        while (sum > 100) sum -= 1;
        return sum % 7;
    }

    function describe(entry) {
        const shape = { name: entry?.name ?? "", tags: [] };
        const upper = entry.name.trim().toUpperCase();
        return names.some(n => n.startsWith(upper)) || !shape.name ? "known" : "new" + upper;
    }

    onCountChanged: {
        if (count > 3)
            root.finished("many");
        count--;
        root.busy = true;
    }
}
