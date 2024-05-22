import 'dart:collection';

import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/ffi.dart';

abstract class Subscriber {
  void notify(Event event);
}

class EventService {
  HashMap<Type, List<Subscriber>> subscribers = HashMap();

  EventService.create() {
    api.subscribe().listen((Event event) {
      if (subscribers[event.runtimeType] == null) {
        logger.d("found no subscribers, skipping event");
        return;
      }

      for (final subscriber in subscribers[event.runtimeType]!) {
        subscriber.notify(event);
      }
    });
  }

  /// Subscribes to an Event based on a dummy Event that we can derive the runtime type from
  ///
  /// This is done because the Event sent by rust is implemented using Inheritance and multiple subclasses.
  /// With this we achieve a dynamic interface where we can subscribe to events based on their runtime type.
  /// In order to be able to derive the runtime type properly we have to actually construct the Event like it is constructed through the bridge.
  void subscribe(Subscriber subscriber, Event event) {
    Type eventType = event.runtimeType;

    if (subscribers[eventType] == null) {
      subscribers[eventType] = List.empty(growable: true);
    }

    logger.i("Subscribed: $subscriber for event: $event $eventType");
    subscribers[eventType]!.add(subscriber);
  }
}

class AnonSubscriber implements Subscriber {
  final Function(dynamic event) notifyFn;

  AnonSubscriber(this.notifyFn);

  @override
  void notify(Event event) {
    notifyFn(event);
  }
}
