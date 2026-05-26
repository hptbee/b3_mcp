import SwiftUI

@main
struct DemoApp: App {
    var body: some Scene {
        WindowGroup { ContentView() }
    }
}

func loadOrders() {
    URLSession.shared.dataTask(with: URL(string: "https://example.test/orders")!)
}
