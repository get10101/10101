import 'package:flutter/widgets.dart';
import 'package:get_10101/ffi.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/poll_service.dart';

class PollChangeNotifier extends ChangeNotifier {
  final PollService service;
  Poll? _polls;

  Poll? get poll => _polls;

  PollChangeNotifier(this.service) {
    refresh();
  }

  Future<void> refresh() async {
    try {
      final poll = await service.fetchPoll();
      _polls = poll;

      super.notifyListeners();
    } catch (error) {
      logger.e(error);
    }
  }

  Future<void> answer(Choice answer, Poll poll) async {
    await service.postAnswer(answer, poll);
    refresh();
  }

  Future<void> ignore(Poll poll) async {
    service.ignorePoll(poll.id);
    refresh();
  }
}
