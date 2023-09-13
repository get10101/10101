import 'package:f_logs/model/flog/flog.dart';
import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/firebase_options.dart';

/// Ask the user for permission to send notifications via Firebase
Future<void> requestNotificationPermission() async {
  FirebaseMessaging messaging = FirebaseMessaging.instance;
  NotificationSettings settings = await messaging.requestPermission(
    alert: true,
    announcement: false,
    badge: true,
    carPlay: false,
    criticalAlert: false,
    provisional: false,
    sound: true,
  );

  FLog.info(text: "User granted permission: ${settings.authorizationStatus}");
}

Future<void> ensureStoringFirebaseToken() async {
  FirebaseMessaging messaging = FirebaseMessaging.instance;
  messaging.getToken().then((token) {
    if (token != null) {
      FLog.info(text: "Firebase token: $token");
      updateFcmToken(token).then((_) {
        FLog.info(text: "Firebase token updated");
      });
    } else {
      FLog.warning(text: "Firebase token is null");
    }
  });

  // Firebase sometimes updates tokens at runtime, make sure we handle that case
  messaging.onTokenRefresh.listen((String token) {
    updateFcmToken(token).then((value) {
      FLog.info(text: "Firebase token updated");
    });
  });
}

Future<void> updateFcmToken(String token) async {
  FLog.debug(text: "Firebase token: $token");
  try {
    await rust.api.updateFcmToken(fcmToken: token);
  } catch (e) {
    FLog.error(text: "Error storing FCM token: ${e.toString()}");
  }
}

Future<void> initFirebase() async {
  try {
    FLog.info(text: "Initialising Firebase");
    await Firebase.initializeApp(
      options: DefaultFirebaseOptions.currentPlatform,
    );
    await ensureStoringFirebaseToken();
    await requestNotificationPermission();
    final flutterLocalNotificationsPlugin = initLocalNotifications();
    await configureFirebase(flutterLocalNotificationsPlugin);
  } catch (e) {
    FLog.error(text: "Error setting up Firebase: ${e.toString()}");
  }
}

Future<void> configureFirebase(FlutterLocalNotificationsPlugin localNotifications) async {
  // Configure message handler
  FirebaseMessaging.onMessage.listen((RemoteMessage message) {
    // TODO: Handle messages from Firebase
    FLog.debug(text: "Firebase message received: ${message.data}");

    if (message.notification != null) {
      FLog.debug(text: "Message also contained a notification: ${message.notification}");
      showNotification(message.notification!.toMap(), localNotifications);
    }
  });

  // Setup the message handler when the app is not running
  FirebaseMessaging.onBackgroundMessage(_firebaseMessagingBackgroundHandler);

  FirebaseMessaging messaging = FirebaseMessaging.instance;
  // Subscribe to topic "all" to receive all messages
  messaging.subscribeToTopic('all');
}

FlutterLocalNotificationsPlugin initLocalNotifications() {
  final flutterLocalNotificationsPlugin = FlutterLocalNotificationsPlugin();
  const androidSettings = AndroidInitializationSettings('@mipmap/ic_launcher');
  const darwinSettings = DarwinInitializationSettings();
  const initializationSettings =
      InitializationSettings(android: androidSettings, macOS: darwinSettings, iOS: darwinSettings);
  flutterLocalNotificationsPlugin.initialize(initializationSettings);
  return flutterLocalNotificationsPlugin;
}

/// Handle background messages (when the app is not running)
Future<void> _firebaseMessagingBackgroundHandler(RemoteMessage message) async {
  FLog.debug(text: "Handling a background message: ${message.messageId}");

  await Firebase.initializeApp();
  final localNotifications = initLocalNotifications();

  if (message.notification != null) {
    FLog.debug(text: "Message also contained a notification: ${message.notification}");
    showNotification(message.notification!.toMap(), localNotifications);
  }
}

/// Display notification inside the `message` using the local notification plugin
void showNotification(
    Map<String, dynamic> message, FlutterLocalNotificationsPlugin localNotifications) async {
  const androidPlatformChannelSpecifics = AndroidNotificationDetails(
    'channel_id',
    'channel_name',
    channelDescription: 'channel_description',
    importance: Importance.max,
    priority: Priority.high,
  );

  const darwinPlatformChannelSpecifics = DarwinNotificationDetails(
    presentAlert: true,
    presentBadge: true,
    presentSound: true,
  );

  const platformChannelSpecifics = NotificationDetails(
    android: androidPlatformChannelSpecifics,
    iOS: darwinPlatformChannelSpecifics,
    macOS: darwinPlatformChannelSpecifics,
  );

  FLog.debug(text: "Showing notification: ${message['title']} with body ${message['body']}");

  await localNotifications.show(
    0,
    message['title'],
    message['body'],
    platformChannelSpecifics,
    payload: 'item x',
  );
}
