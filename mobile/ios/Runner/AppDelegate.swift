import UIKit
import Flutter
import workmanager

@UIApplicationMain
@objc class AppDelegate: FlutterAppDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    print("dummy_value=\(dummy_method_to_enforce_bundling())");

    // Set the delegate for the UNUserNotificationCenter
    if #available(iOS 10.0, *) {
        UNUserNotificationCenter.current().delegate = self
    }
    GeneratedPluginRegistrant.register(with: self)

    // Register background task
    WorkmanagerPlugin.registerTask(withIdentifier: "task-identifier")

    // Don't try to do background fetches more often than every 15 mins
    // (unfortunately, we cannot specify how often they will be run at the minimum)
    UIApplication.shared.setMinimumBackgroundFetchInterval(TimeInterval(60*15))

    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
}

  // Implement the method to handle the display of notifications in the foreground
  @available(iOS 10.0, *)
  override func userNotificationCenter(_ center: UNUserNotificationCenter,
                              willPresent notification: UNNotification,
                              withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
      completionHandler([.sound, .alert, .badge])
  }
}
