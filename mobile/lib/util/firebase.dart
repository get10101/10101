import 'package:flutter/foundation.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_crashlytics/firebase_crashlytics.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:get_10101/firebase_options.dart';
import 'package:get_10101/util/environment.dart';

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

  logger.i("User granted permission: ${settings.authorizationStatus}");
}

Future<void> initFirebase() async {
  final env = Environment.parse();

  try {
    logger.i("Initialising Firebase");

    await Firebase.initializeApp(
      options: DefaultFirebaseOptions(env.network).currentPlatform,
    );
  } catch (e) {
    logger.e("Error initializing Firebase: ${e.toString()}");
  }

  try {
    logger.i("Setting up Firebase notifications");

    await requestNotificationPermission();
    final flutterLocalNotificationsPlugin = initLocalNotifications();
    await configureFirebase(flutterLocalNotificationsPlugin);
  } catch (e) {
    logger.e("Error setting up Firebase notifications: ${e.toString()}");
  }

  try {
    logger.i("Setting up Firebase Crashlytics");

    FlutterError.onError = (errorDetails) {
      FirebaseCrashlytics.instance.recordFlutterFatalError(errorDetails);
    };
    // Pass all uncaught asynchronous errors that aren't handled by the Flutter framework to Crashlytics
    PlatformDispatcher.instance.onError = (error, stack) {
      FirebaseCrashlytics.instance.recordError(error, stack, fatal: true);
      return true;
    };
  } catch (e) {
    logger.e("Error setting up Firebase Crashlytics: ${e.toString()}");
  }
}

Future<void> configureFirebase(FlutterLocalNotificationsPlugin localNotifications) async {
  // Configure message handler
  FirebaseMessaging.onMessage.listen((RemoteMessage message) {
    // TODO: Handle messages from Firebase
    logger.d("Firebase message received: ${message.data}");

    if (message.notification != null) {
      logger.d("Message also contained a notification: ${message.notification}");
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
  logger.d("Handling a background message: ${message.messageId}");

  await Firebase.initializeApp();
  final localNotifications = initLocalNotifications();

  if (message.notification != null) {
    logger.d("Message also contained a notification: ${message.notification}");
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

  logger.d("Showing notification: ${message['title']} with body ${message['body']}");

  await localNotifications.show(
    0,
    message['title'],
    message['body'],
    platformChannelSpecifics,
    payload: 'item x',
  );
}
