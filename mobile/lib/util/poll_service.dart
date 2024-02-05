import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/logger/logger.dart';

class PollService {
  const PollService();

  Future<rust.Poll?> fetchPoll() async {
    try {
      return await rust.api.fetchPoll();
    } catch (error) {
      logger.e("Failed to fetch polls: $error");
      return null;
    }
  }

  Future<void> postAnswer(rust.Choice choice, rust.Poll poll) async {
    return await rust.api.postSelectedChoice(pollId: poll.id, selectedChoice: choice);
  }

  void ignorePoll(int pollId) {
    return rust.api.ignorePoll(pollId: pollId);
  }
}
