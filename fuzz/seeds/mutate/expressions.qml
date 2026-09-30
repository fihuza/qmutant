import QtQuick
Item { x: a?.b ?? [1, 2].some(v => v > 0) ? "y" : `z` }
