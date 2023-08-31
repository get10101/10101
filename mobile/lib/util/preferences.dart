import 'package:f_logs/f_logs.dart';
import 'package:shared_preferences/shared_preferences.dart';

class Preferences {
  Preferences._privateConstructor();

  static final Preferences instance = Preferences._privateConstructor();

  static const userSeedBackupConfirmed = "userSeedBackupConfirmed";
  static const emailAddress = "emailAddress";

  // Position expiry is used by the background task to check whether we should show a notification
  static const positionExpiry = "positionExpiry";

  setUserSeedBackupConfirmed() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setBool(userSeedBackupConfirmed, true);
  }

  Future<bool> isUserSeedBackupConfirmed() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getBool(userSeedBackupConfirmed) ?? false;
  }

  setEmailAddress(String value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(emailAddress, value);
  }

  Future<String> getEmailAddress() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(emailAddress) ?? "";
  }

  Future<bool> hasEmailAddress() async {
    var email = await getEmailAddress();
    return email.isNotEmpty;
  }

  // Note: this is not async as it needs to be used in background handler
  DateTime? getPositionExpiry() {
    SharedPreferences.getInstance().then((preferences) {
      var expiry = preferences.getInt(positionExpiry);
      if (expiry == null) {
        return null;
      }
      return DateTime.fromMillisecondsSinceEpoch(expiry);
    });
  }

  setPositionExpiry(DateTime value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setInt(positionExpiry, value.millisecondsSinceEpoch);
  }

  clearPositionExpiry() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    if (!await preferences.remove(positionExpiry)) {
      FLog.warning(text: "Failed to remove positionExpiry");
    }
  }
}
